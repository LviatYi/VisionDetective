use crate::GameStatus;
use crate::GameView;
use crate::asset::runtime_root;
use crate::card::Card;
use crate::card::card_params::{CardParam, SpawnCardSystemParams};
use crate::card::spawn_card_by_card_param;
use crate::coin::character::{CharacterCoinParam, spawn_character_coin};
use crate::config::GameConfig;
use crate::config::character_config::CharacterConfig;
use crate::progress::GameProgress;
use crate::scene::terrain::{TerrainParam, spawn_terrain};
use bevy::prelude::*;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

const DEMO_SCENE_PATH: &str = "assets/scene/scene-demo-01.toml";

#[derive(Deserialize, Default)]
struct SceneFile {
    #[serde(default)]
    cards: Vec<CardParam>,

    #[serde(default)]
    terrains: Vec<TerrainParam>,

    #[serde(default)]
    character_coins: Vec<CharacterCoinParam>,
}

#[derive(Resource, Default)]
pub struct RuntimeSceneCards {
    cards: Vec<RuntimeSceneCard>,
    character_coins: Vec<RuntimeSceneCharacterCoin>,
}

struct RuntimeSceneCard {
    param: CardParam,
    entity: Option<Entity>,
}

struct RuntimeSceneCharacterCoin {
    param: CharacterCoinParam,
    entity: Option<Entity>,
}

#[derive(Component)]
struct RuntimeSceneCardEntity;

pub struct RuntimeScenePlugin;

impl Plugin for RuntimeScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RuntimeSceneCards>();
        app.add_systems(OnEnter(GameStatus::DeckEntering), setup_deck_entering);
        app.add_systems(
            Update,
            update_deck_entering.run_if(in_state(GameStatus::DeckEntering)),
        );
        app.add_systems(OnEnter(GameStatus::Dealing), setup_card_dealing);
        app.add_systems(
            Update,
            update_card_dealing.run_if(in_state(GameStatus::Dealing)),
        );
        app.add_systems(
            OnEnter(GameStatus::PlayerEntering),
            spawn_runtime_character_coins,
        );
    }
}

pub fn load_demo_scene(
    commands: &mut Commands,
    spawn_params: &mut SpawnCardSystemParams<'_>,
    _progress: &GameProgress,
    runtime_cards: &mut RuntimeSceneCards,
) {
    runtime_cards.cards.clear();

    let scene_path = demo_scene_path();
    let scene = match fs::read_to_string(&scene_path)
        .map_err(|error| error.to_string())
        .and_then(|raw| toml::from_str::<SceneFile>(&raw).map_err(|error| error.to_string()))
    {
        Ok(scene) => scene,
        Err(error) => {
            bevy::log::error!(
                "failed to load demo scene {}: {error}",
                scene_path.display()
            );
            return;
        }
    };

    runtime_cards.cards = scene
        .cards
        .into_iter()
        .map(|param| RuntimeSceneCard {
            param,
            entity: None,
        })
        .collect();
    runtime_cards.character_coins = scene
        .character_coins
        .into_iter()
        .map(|param| RuntimeSceneCharacterCoin {
            param,
            entity: None,
        })
        .collect();
    spawn_runtime_scene_cards(commands, spawn_params, runtime_cards);

    for terrain in &scene.terrains {
        spawn_terrain(commands, spawn_params, terrain);
    }
}

fn spawn_runtime_scene_cards(
    commands: &mut Commands,
    spawn_params: &mut SpawnCardSystemParams<'_>,
    runtime_cards: &mut RuntimeSceneCards,
) {
    for runtime_card in &mut runtime_cards.cards {
        if runtime_card.entity.is_some() {
            continue;
        }

        let entity = spawn_card_by_card_param(commands, spawn_params, &runtime_card.param, false);
        commands
            .entity(entity)
            .insert((GameView, RuntimeSceneCardEntity));
        runtime_card.entity = Some(entity);
    }
}

fn demo_scene_path() -> PathBuf {
    runtime_root().join(DEMO_SCENE_PATH)
}

fn spawn_runtime_character_coins(
    mut commands: Commands,
    mut spawn_params: SpawnCardSystemParams<'_>,
    character_config: Res<CharacterConfig>,
    mut runtime_cards: ResMut<RuntimeSceneCards>,
) {
    for character_coin in &mut runtime_cards.character_coins {
        if character_coin.entity.is_some() {
            continue;
        }

        character_coin.entity = spawn_character_coin(
            &mut commands,
            spawn_params.asset_server.as_ref(),
            spawn_params.meshes.as_mut(),
            spawn_params.materials.as_mut(),
            &*spawn_params.config,
            &character_config,
            &character_coin.param,
            GameView,
        );
    }
}

#[derive(Component)]
struct DeckEnteringAnimation {
    elapsed: f32,
    start_translation: Vec3,
    target_translation: Vec3,
}

#[derive(Component)]
struct DeckCardTarget {
    translation: Vec3,
    rotation: Quat,
}

#[derive(Component)]
struct CardDealingAnimation {
    delay: f32,
    elapsed: f32,
    start_translation: Vec3,
    target_translation: Vec3,
    start_rotation: Quat,
    target_rotation: Quat,
}

fn setup_deck_entering(
    mut commands: Commands,
    config: Res<GameConfig>,
    card_query: Query<(Entity, &Transform), (With<Card>, With<RuntimeSceneCardEntity>)>,
) {
    let deck_position = Vec2::new(
        config.scene.card_dealing_deck_position[0],
        config.scene.card_dealing_deck_position[1],
    );

    for (index, (entity, transform)) in card_query.iter().enumerate() {
        let card_target = DeckCardTarget {
            translation: transform.translation,
            rotation: transform.rotation,
        };
        let target_translation = deck_position
            .extend(transform.translation.z + index as f32 * CARD_DEALING_STACK_Z_OFFSET);
        let start_translation =
            (deck_position + DECK_ENTERING_START_OFFSET).extend(target_translation.z);
        commands.entity(entity).insert((
            Transform {
                translation: start_translation,
                rotation: Quat::IDENTITY,
                scale: transform.scale,
            },
            DeckEnteringAnimation {
                elapsed: 0.0,
                start_translation,
                target_translation,
            },
            card_target,
        ));
    }
}

fn update_deck_entering(
    mut commands: Commands,
    time: Res<Time>,
    mut next_game_state: ResMut<NextState<GameStatus>>,
    mut card_query: Query<(Entity, &mut DeckEnteringAnimation, &mut Transform)>,
) {
    let mut any_animating = false;

    for (entity, mut animation, mut transform) in &mut card_query {
        animation.elapsed += time.delta_secs();
        let progress = (animation.elapsed / DECK_ENTERING_DURATION).clamp(0.0, 1.0);
        let eased = smoothstep(progress);
        transform.translation = animation
            .start_translation
            .lerp(animation.target_translation, eased);
        transform.rotation = Quat::IDENTITY;

        if progress >= 1.0 {
            transform.translation = animation.target_translation;
            commands.entity(entity).remove::<DeckEnteringAnimation>();
        } else {
            any_animating = true;
        }
    }

    if !any_animating {
        next_game_state.set(GameStatus::Dealing);
    }
}

fn setup_card_dealing(
    mut commands: Commands,
    config: Res<GameConfig>,
    card_query: Query<
        (Entity, &Transform, &DeckCardTarget),
        (With<Card>, With<RuntimeSceneCardEntity>),
    >,
) {
    let player_position = Vec2::ZERO;
    let mut cards = card_query.iter().collect::<Vec<_>>();
    cards.sort_by(|(entity_a, _, target_a), (entity_b, _, target_b)| {
        let distance_a = target_a
            .translation
            .truncate()
            .distance_squared(player_position);
        let distance_b = target_b
            .translation
            .truncate()
            .distance_squared(player_position);

        distance_b
            .partial_cmp(&distance_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| entity_a.index().cmp(&entity_b.index()))
    });
    let interval = CARD_DEALING_TOTAL_DURATION / cards.len().max(1) as f32;
    let deck_position = Vec2::new(
        config.scene.card_dealing_deck_position[0],
        config.scene.card_dealing_deck_position[1],
    );

    for (index, (entity, transform, target)) in cards.into_iter().enumerate() {
        let target_translation = target.translation;
        let target_rotation = target.rotation;
        let start_translation =
            deck_position.extend(target_translation.z + index as f32 * CARD_DEALING_STACK_Z_OFFSET);
        let start_rotation = Quat::IDENTITY;

        commands.entity(entity).insert((
            Transform {
                translation: start_translation,
                rotation: start_rotation,
                scale: transform.scale,
            },
            CardDealingAnimation {
                delay: index as f32 * interval,
                elapsed: 0.0,
                start_translation,
                target_translation,
                start_rotation,
                target_rotation,
            },
        ));
    }
}

fn update_card_dealing(
    mut commands: Commands,
    time: Res<Time>,
    mut next_game_state: ResMut<NextState<GameStatus>>,
    mut card_query: Query<(Entity, &mut CardDealingAnimation, &mut Transform)>,
) {
    let mut any_animating = false;

    for (entity, mut animation, mut transform) in &mut card_query {
        animation.elapsed += time.delta_secs();
        let active_elapsed = animation.elapsed - animation.delay;

        if active_elapsed <= 0.0 {
            any_animating = true;
            continue;
        }

        let progress = (active_elapsed / CARD_DEALING_DURATION).clamp(0.0, 1.0);
        let eased = smoothstep(progress);
        transform.translation = animation
            .start_translation
            .lerp(animation.target_translation, eased);
        transform.rotation = animation
            .start_rotation
            .slerp(animation.target_rotation, eased);

        if progress >= 1.0 {
            transform.translation = animation.target_translation;
            transform.rotation = animation.target_rotation;
            commands.entity(entity).remove::<CardDealingAnimation>();
        } else {
            any_animating = true;
        }
    }

    if !any_animating {
        next_game_state.set(GameStatus::PlayerEntering);
    }
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

const CARD_DEALING_DURATION: f32 = 0.5;
const CARD_DEALING_TOTAL_DURATION: f32 = 3.0;
const CARD_DEALING_STACK_Z_OFFSET: f32 = 0.0001;
const DECK_ENTERING_DURATION: f32 = 0.75;
const DECK_ENTERING_START_OFFSET: Vec2 = Vec2::new(-900.0, 0.0);
