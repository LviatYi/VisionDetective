use crate::GameplaySet;
#[cfg(not(target_arch = "wasm32"))]
use crate::asset::runtime_root;
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::{PlayerCoinState, RefPlayerCoinStateExt};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
const GAME_PROGRESS_SAVE_PATH: &str = "game-progress.toml";

#[derive(Resource, Default, Serialize, Deserialize)]
pub struct GameProgress {
    #[serde(default)]
    pub unlocked_conditions: HashSet<String>,

    #[serde(default)]
    pub last_player_stop_position: Option<Vec2>,

    #[serde(default)]
    pub revealed_clue_instances: HashSet<String>,
}

impl GameProgress {
    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Self {
        Self::default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Self {
        let path = Self::save_path();
        let Ok(raw) = fs::read_to_string(&path) else {
            return Self::default();
        };

        toml::from_str(&raw).unwrap_or_else(|error| {
            warn!("failed to parse progress save {}: {error}", path.display());
            Self::default()
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save(&self) {}

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self) {
        let path = Self::save_path();
        match toml::to_string_pretty(self) {
            Ok(raw) => {
                if let Err(error) = fs::write(&path, raw) {
                    warn!("failed to write progress save {}: {error}", path.display());
                }
            }
            Err(error) => warn!(
                "failed to serialize progress save {}: {error}",
                path.display()
            ),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn delete_save() {}

    #[cfg(not(target_arch = "wasm32"))]
    pub fn delete_save() {
        let path = Self::save_path();
        if path.exists()
            && let Err(error) = fs::remove_file(&path)
        {
            warn!("failed to delete progress save {}: {error}", path.display());
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn has_save() -> bool {
        false
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn has_save() -> bool {
        Self::save_path().exists()
    }

    pub fn unlock(&mut self, key: impl AsRef<str>) -> bool {
        let key = key.as_ref().to_string();
        if self.unlocked_conditions.contains(&key) {
            return false;
        }
        self.unlocked_conditions.insert(key);
        self.save();
        true
    }

    pub fn reveal_clue(&mut self, instance_id: impl AsRef<str>) -> bool {
        let inserted = self
            .revealed_clue_instances
            .insert(instance_id.as_ref().to_string());
        if inserted {
            self.save();
        }
        inserted
    }

    pub fn is_unlocked(&self, key: &str) -> bool {
        self.unlocked_conditions
            .iter()
            .any(|condition| condition == key)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_path() -> PathBuf {
        runtime_root().join(GAME_PROGRESS_SAVE_PATH)
    }
}

pub struct GameProgressPlugin;

impl Plugin for GameProgressPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameProgress::load()).add_systems(
            Update,
            record_player_stop_position.in_set(GameplaySet::PlayerRecordProgress),
        );
    }
}

fn record_player_stop_position(
    player_query: Query<(Ref<PlayerCoinState>, &Transform), With<PlayerCoin>>,
    mut progress: ResMut<GameProgress>,
) {
    for (player_state, transform) in player_query.iter() {
        if !player_state.just_eject_finished_or_initialized() {
            continue;
        }

        let position = transform.translation.truncate();
        if progress.last_player_stop_position != Some(position) {
            progress.last_player_stop_position = Some(position);
            progress.save();
        }
    }
}
