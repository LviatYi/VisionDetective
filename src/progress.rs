use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::PlayerCoinState;
use crate::game_view::GameplaySet;
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
            record_player_stop_position
                .after(crate::physics::move_player_coin_transform)
                .in_set(GameplaySet::PlayerPhysics),
        );
    }
}

fn record_player_stop_position(
    player_state: Res<PlayerCoinState>,
    player_query: Query<&Transform, With<PlayerCoin>>,
    mut progress: ResMut<GameProgress>,
) {
    if !player_state.is_changed() || !player_state.is_stop() {
        return;
    }

    let Ok(transform) = player_query.single() else {
        return;
    };

    progress.last_player_stop_position = Some(transform.translation.truncate());
}
