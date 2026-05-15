use crate::GameplaySet;
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::{PlayerCoinState, RefPlayerCoinStateExt};
use bevy::prelude::*;
use std::collections::HashSet;

#[derive(Resource, Default)]
pub struct GameProgress {
    pub unlocked_conditions: HashSet<String>,

    pub last_player_stop_position: Option<Vec2>,

    pub revealed_clue_instances: HashSet<String>,
}

impl GameProgress {
    pub fn unlock(&mut self, key: impl AsRef<str>) -> bool {
        let key = key.as_ref().to_string();
        if self.unlocked_conditions.contains(&key) {
            return false;
        }
        self.unlocked_conditions.insert(key);
        true
    }

    pub fn is_unlocked(&self, key: &str) -> bool {
        self.unlocked_conditions
            .iter()
            .any(|condition| condition == key)
    }
}

pub struct GameProgressPlugin;

impl Plugin for GameProgressPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameProgress>().add_systems(
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

        progress.last_player_stop_position = Some(transform.translation.truncate());
    }
}
