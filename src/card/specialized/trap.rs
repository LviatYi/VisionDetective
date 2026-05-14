use crate::card::card_params::{CardSpecializedParam, SpawnCardSystemParams};
use crate::card::{Card, CardKind, CardSpecializedInstaller};
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::{PlayerCoinBehaviorStatus, PlayerCoinState};
use crate::physics::area::{Area, ShapeType};
use crate::tools::Disable;
use crate::{GameplaySet, register_card_specialized_installer};
use bevy::app::{App, Update};
use bevy::math::Vec2;
use bevy::prelude::{
    Color, Component, Deref, Entity, EntityCommands, Gizmos, GlobalTransform, IntoScheduleConfigs,
    Mut, Query, Transform, With, Without,
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
                .in_set(GameplaySet::PlayerDeath),
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

    fn spawn_with(
        &self,
        entity: &mut EntityCommands<'_>,
        spawn_params: &mut SpawnCardSystemParams<'_>,
    ) {
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
    trap_query: Query<(Entity, &Transform, &Trap), Without<Disable>>,
    card_query: Query<(Entity, &Card, &GlobalTransform), Without<Disable>>,
) {
    for (mut player_state, player_transform) in &mut player_query {
        if !player_state.is_on_ground() {
            continue;
        }

        let player_position = player_transform.translation.truncate();
        let falls_into_trap = trap_query
            .iter()
            .filter(|(_, trap_transform, trap)| {
                trap.contains_world_point(trap_transform, player_position)
            })
            .any(|(trap_entity, trap_transform, _)| {
                !is_covered_by_higher_card(
                    trap_entity,
                    trap_transform.translation.z,
                    player_position,
                    &card_query,
                )
            });

        if falls_into_trap {
            player_state.set_state(PlayerCoinBehaviorStatus::Death);
        }
    }
}

pub fn is_covered_by_higher_card(
    trap_entity: Entity,
    trap_z: f32,
    player_position: Vec2,
    card_query: &Query<(Entity, &Card, &GlobalTransform), Without<Disable>>,
) -> bool {
    card_query
        .iter()
        .filter(|(entity, _, transform)| {
            *entity != trap_entity && transform.translation().z > trap_z
        })
        .any(|(_, card, transform)| card.contains_point(transform, player_position))
}

//region Gizmos

pub fn draw_trap_paths(mut gizmos: Gizmos, trap_query: Query<(&Transform, &Trap)>) {
    for (transform, trap) in &trap_query {
        let world_path = trap.world_path(transform);
        if world_path.len() < 2 {
            continue;
        }

        for index in 0..world_path.len() {
            let a = world_path[index];
            let b = world_path[(index + 1) % world_path.len()];
            gizmos.line_2d(a, b, Color::srgb(0.91, 0.42, 0.42));
            gizmos.circle_2d(a, 3.0, Color::srgb(1.0, 0.72, 0.76));
        }
    }
}

//endregion
