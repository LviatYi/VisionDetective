use crate::card::card_params::{CardSpawnParams, CardSpecializedParam};
use crate::card::{CardKind, CardSpecializedInstaller};
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::{PlayerCoinBehaviorStatus, PlayerCoinState};
use crate::config::GameConfig;
use crate::physics::area::{Area, ShapeType};
use crate::tools::Disable;
use crate::{GameplaySet, register_card_specialized_installer};
use bevy::app::{App, Update};
use bevy::math::Vec2;
use bevy::prelude::{
    Component, Deref, EntityCommands, Gizmos, IntoScheduleConfigs, Mut, Query, Res, Transform,
    With, Without,
};
use serde::{Deserialize, Serialize};

//region Installer

pub struct TrapCardSpecializedInstaller;

impl CardSpecializedInstaller for TrapCardSpecializedInstaller {
    type Param = TrapCardParams;

    const TYPE_ID: &'static str = "trap";

    fn install(app: &mut App) {
        app.add_systems(
            Update,
            handle_player_trap_collision
                .after(crate::physics::move_player_coin_transform)
                .in_set(GameplaySet::PlayerPhysics),
        );
    }
}

register_card_specialized_installer!(TrapCardSpecializedInstaller);

//endregion

//region Card Specialized Param

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapCardParams {
    pub shape_def: ShapeType,
}

impl CardSpecializedParam for TrapCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Trap
    }

    fn spawn_with(&self, entity: &mut EntityCommands<'_>, spawn_params: &mut CardSpawnParams<'_>) {
        entity.insert(Trap::new(
            self.shape_def.sample_path(&spawn_params.config.cards),
        ));
    }
}

//endregion

//region Component

#[derive(Component, Clone, Deref)]
pub struct Trap {
    pub area: Area,
}

impl Trap {
    pub fn new(local_path: Vec<Vec2>) -> Self {
        Self {
            area: Area { local_path },
        }
    }
}

//endregion

fn handle_player_trap_collision(
    mut player_query: Query<(Mut<PlayerCoinState>, &Transform), With<PlayerCoin>>,
    trap_query: Query<(&Transform, &Trap), Without<Disable>>,
) {
    for (mut player_state, player_transform) in &mut player_query {
        if !player_state.is_on_ground() {
            continue;
        }

        let player_position = player_transform.translation.truncate();
        if trap_query.iter().any(|(trap_transform, trap)| {
            trap.contains_world_point(trap_transform, player_position)
        }) {
            player_state.set_state(PlayerCoinBehaviorStatus::Death);
        }
    }
}

//region Gizmos

pub fn draw_trap_paths(
    config: Res<GameConfig>,
    mut gizmos: Gizmos,
    trap_query: Query<(&Transform, &Trap)>,
) {
    for (transform, trap) in &trap_query {
        let world_path = trap.world_path(transform);
        if world_path.len() < 2 {
            continue;
        }

        todo!("//TODO_LviatYi: by Lviat Yi");
        // for index in 0..world_path.len() {
        //     let a = world_path[index];
        //     let b = world_path[(index + 1) % world_path.len()];
        //     gizmos.line_2d(a, b, config.traps.edge_color());
        //     gizmos.circle_2d(
        //         a,
        //         config.traps.vertex_radius,
        //         config.traps.vertex_color(),
        //     );
        // }
    }
}

//endregion
