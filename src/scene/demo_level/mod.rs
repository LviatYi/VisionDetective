use crate::GameView;
use crate::card::card_params::{CardParam, CardSpawnParams};
use crate::card::spawn_card_by_card_param;
use crate::game_view::GameState;
use crate::progress::GameProgress;
use bevy::prelude::*;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

const DEMO_SCENE_PATH: &str = "assets/scene/scene-demo-01.toml";

#[derive(Deserialize, Default)]
struct SceneFile {
    #[serde(default)]
    cards: Vec<CardParam>,
}

#[derive(Resource, Default)]
pub struct RuntimeSceneCards {
    cards: Vec<RuntimeSceneCard>,
}

struct RuntimeSceneCard {
    param: CardParam,
    entity: Option<Entity>,
}

pub struct RuntimeScenePlugin;

impl Plugin for RuntimeScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RuntimeSceneCards>().add_systems(
            Update,
            sync_runtime_scene_cards.run_if(in_state(GameState::InGame)),
        );
    }
}

pub fn load_demo_scene(
    commands: &mut Commands,
    spawn_params: &mut CardSpawnParams<'_>,
    progress: &GameProgress,
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
    spawn_runtime_scene_cards(commands, spawn_params, progress, runtime_cards);
}

fn sync_runtime_scene_cards(
    mut commands: Commands,
    mut spawn_params: CardSpawnParams<'_>,
    progress: Res<GameProgress>,
    mut runtime_cards: ResMut<RuntimeSceneCards>,
) {
    if !progress.is_changed() {
        return;
    }

    despawn_runtime_scene_cards(&mut commands, &progress, &mut runtime_cards);
    spawn_runtime_scene_cards(
        &mut commands,
        &mut spawn_params,
        &progress,
        &mut runtime_cards,
    );
}

fn spawn_runtime_scene_cards(
    commands: &mut Commands,
    spawn_params: &mut CardSpawnParams<'_>,
    progress: &GameProgress,
    runtime_cards: &mut RuntimeSceneCards,
) {
    for runtime_card in &mut runtime_cards.cards {
        if runtime_card.entity.is_some() || !card_should_spawn(&runtime_card.param, progress) {
            continue;
        }

        let entity = spawn_card_by_card_param(commands, spawn_params, &runtime_card.param);
        commands.entity(entity).insert(GameView);
        runtime_card.entity = Some(entity);
    }
}

fn despawn_runtime_scene_cards(
    commands: &mut Commands,
    progress: &GameProgress,
    runtime_cards: &mut RuntimeSceneCards,
) {
    for runtime_card in &mut runtime_cards.cards {
        if runtime_card.entity.is_none() || card_should_spawn(&runtime_card.param, progress) {
            continue;
        }

        if let Some(entity) = runtime_card.entity.take() {
            commands.entity(entity).despawn();
        }
    }
}

fn card_should_spawn(card: &CardParam, progress: &GameProgress) -> bool {
    let spawn_unlocked = card
        .scene_param
        .spawn_if
        .as_deref()
        .map(|key| progress.is_unlocked(key))
        .unwrap_or(true);
    let destroy_unlocked = card
        .scene_param
        .destroy_if
        .as_deref()
        .map(|key| progress.is_unlocked(key))
        .unwrap_or(false);

    spawn_unlocked && !destroy_unlocked
}

fn demo_scene_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEMO_SCENE_PATH)
}
