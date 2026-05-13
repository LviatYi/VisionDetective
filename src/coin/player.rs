use crate::coin::player::controller::{
    CursorWorldPosition, draw_arena_and_aim, finish_player_charge_from_pointer,
    handle_player_pointer_drag, start_player_charge_from_pointer, track_pointer_world_position,
    update_aiming_marker, update_player_hover_state, update_player_visuals,
};
use crate::input::player_input_allowed;
use crate::{GameLoadingSet, GameStatus, GameplaySet};
use bevy::prelude::*;

#[derive(Component, Default)]
pub struct PlayerCoin;

pub struct PlayerPlugin;

pub mod controller {
    use crate::coin::player::PlayerCoin;
    use crate::config::GameConfig;
    use crate::physics::Velocity;
    use crate::progress::GameProgress;
    use crate::scene::SceneLayer;
    use bevy::math::{Vec2, Vec3};
    use bevy::picking::pointer::PointerButton;
    use bevy::picking::prelude::{Drag, DragEnd, Move, Out, Over, Pointer, Press, Release};
    use bevy::prelude::{
        Assets, Camera, Camera2d, Circle, ColorMaterial, Commands, Component, DetectChanges,
        Entity, Gizmos, GlobalTransform, Mesh, Mesh2d, MeshMaterial2d, MessageReader, Pickable,
        Query, Ref, Res, ResMut, Resource, Transform, Visibility, With,
    };
    use std::collections::HashSet;
    use std::ops::{Deref, DerefMut};

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub enum PlayerCoinBehaviorStatus {
        #[default]
        Initialized,
        Idle,
        Aiming,
        Charging {
            eject_vector: Vec2,
        },
        Upspring {
            contact_count: u8,
            sim_z: f32,
        },
        Contact {
            contact_count: u8,
            save_velocity: Vec3,
        },
        Slide,
        Death,
    }

    impl PlayerCoinBehaviorStatus {
        pub fn new_upspring() -> Self {
            Self::Upspring {
                contact_count: 0,
                sim_z: 0.0,
            }
        }

        pub fn is_initialized(&self) -> bool {
            matches!(self, PlayerCoinBehaviorStatus::Initialized)
        }

        pub fn is_idle(&self) -> bool {
            matches!(self, PlayerCoinBehaviorStatus::Idle)
        }

        pub fn is_moving(&self) -> bool {
            matches!(
                self,
                PlayerCoinBehaviorStatus::Upspring { .. }
                    | PlayerCoinBehaviorStatus::Contact { .. }
                    | PlayerCoinBehaviorStatus::Slide
            )
        }

        pub fn is_stop(&self) -> bool {
            !self.is_moving() && !matches!(self, PlayerCoinBehaviorStatus::Death)
        }

        pub fn is_aiming(&self) -> bool {
            matches!(self, PlayerCoinBehaviorStatus::Aiming)
        }

        pub fn is_charging(&self) -> bool {
            matches!(self, PlayerCoinBehaviorStatus::Charging { .. })
        }

        pub fn is_airborne(&self) -> bool {
            matches!(self, PlayerCoinBehaviorStatus::Upspring { .. })
        }

        pub fn is_on_ground(&self) -> bool {
            !self.is_airborne() && !self.is_death()
        }

        pub fn is_death(&self) -> bool {
            matches!(self, PlayerCoinBehaviorStatus::Death)
        }

        pub fn eject_vector(&self) -> Vec2 {
            match self {
                PlayerCoinBehaviorStatus::Charging { eject_vector } => *eject_vector,
                _ => Vec2::ZERO,
            }
        }

        pub fn contact_count(&self) -> u8 {
            match self {
                PlayerCoinBehaviorStatus::Upspring { contact_count, .. }
                | PlayerCoinBehaviorStatus::Contact { contact_count, .. } => *contact_count,
                _ => 0,
            }
        }

        pub fn set_contact_count(&mut self, count: u8) -> bool {
            match self {
                PlayerCoinBehaviorStatus::Upspring { contact_count, .. }
                | PlayerCoinBehaviorStatus::Contact { contact_count, .. } => {
                    *contact_count = count;
                    true
                }
                _ => false,
            }
        }

        pub fn sim_z(&self) -> f32 {
            match self {
                PlayerCoinBehaviorStatus::Upspring { sim_z, .. } => *sim_z,
                _ => 0.0,
            }
        }

        pub fn set_sim_z(&mut self, in_sim_z: f32) -> bool {
            match self {
                PlayerCoinBehaviorStatus::Upspring { sim_z, .. } => {
                    *sim_z = in_sim_z;
                    true
                }
                _ => false,
            }
        }
    }

    #[derive(Component, Default)]
    pub struct PlayerCoinState {
        state: PlayerCoinBehaviorStatus,

        pub last: Option<PlayerCoinBehaviorStatus>,
    }

    impl PlayerCoinState {
        pub fn state(&self) -> PlayerCoinBehaviorStatus {
            self.state
        }

        pub fn set_state(&mut self, state: PlayerCoinBehaviorStatus) {
            self.last = Some(self.state);
            self.state = state;
        }

        pub fn reset(&mut self) {
            self.set_state(PlayerCoinBehaviorStatus::Initialized);
        }
    }

    impl Deref for PlayerCoinState {
        type Target = PlayerCoinBehaviorStatus;

        fn deref(&self) -> &Self::Target {
            &self.state
        }
    }

    impl DerefMut for PlayerCoinState {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.state
        }
    }

    pub trait RefPlayerCoinStateExt {
        fn just_eject_finished(&self) -> bool;

        fn just_initialized(&self) -> bool;

        fn just_eject_finished_or_initialized(&self) -> bool {
            self.just_eject_finished() || self.just_initialized()
        }
    }

    impl<'w> RefPlayerCoinStateExt for Ref<'w, PlayerCoinState> {
        fn just_eject_finished(&self) -> bool {
            self.is_changed()
                && matches!(self.state, PlayerCoinBehaviorStatus::Idle)
                && self.last.is_some_and(|s| s.is_moving())
        }

        fn just_initialized(&self) -> bool {
            self.is_changed()
                && self.state.is_idle()
                && self.last.is_some_and(|s| s.is_initialized())
        }
    }

    #[derive(Resource, Default)]
    pub struct CursorWorldPosition(pub Option<Vec2>);

    pub fn set_player_coin_state_idle(mut player_coin_state: Query<&mut PlayerCoinState>) {
        player_coin_state
            .iter_mut()
            .for_each(|mut state| state.set_state(PlayerCoinBehaviorStatus::Idle));
    }

    pub fn handle_death(
        mut player_query: Query<
            (&mut PlayerCoinState, &mut Transform, &mut Velocity),
            With<PlayerCoin>,
        >,
        game_progress: Res<GameProgress>,
    ) {
        for (mut player_state, mut player_transform, mut velocity) in player_query.iter_mut() {
            if !player_state.is_death() {
                continue;
            }

            player_state.set_state(PlayerCoinBehaviorStatus::Idle);
            player_transform.translation = game_progress
                .last_player_stop_position
                .unwrap_or_else(Vec2::default)
                .extend(SceneLayer::PlayerCoin.get_layer_base_z());
            *velocity = Velocity::default();
        }
    }

    #[derive(Component)]
    pub struct PointerMarker;

    pub fn setup_player(
        mut commands: Commands,
        config: Res<GameConfig>,
        progress: Res<GameProgress>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<ColorMaterial>>,
    ) {
        let player_position = progress.last_player_stop_position.unwrap_or(Vec2::ZERO);
        commands.spawn((
            Mesh2d(meshes.add(Circle::new(config.visuals.player_radius))),
            MeshMaterial2d(materials.add(config.visuals.player_color())),
            Transform::from_translation(Vec3::new(
                player_position.x,
                player_position.y,
                SceneLayer::PlayerCoin.get_layer_base_z(),
            )),
            PlayerCoin::default(),
            Velocity::default(),
            Pickable::default(),
            crate::GameView,
            PlayerCoinState::default(),
        ));

        commands.spawn((
            Mesh2d(meshes.add(Circle::new(config.visuals.pointer_radius))),
            MeshMaterial2d(materials.add(config.visuals.pointer_color())),
            Transform::from_translation(Vec3::new(
                0.0,
                0.0,
                SceneLayer::GizmoAimingMarker.get_layer_base_z(),
            )),
            Visibility::Hidden,
            PointerMarker,
            crate::GameView,
        ));
    }

    pub fn track_pointer_world_position(
        mut move_events: MessageReader<Pointer<Move>>,
        mut over_events: MessageReader<Pointer<Over>>,
        mut press_events: MessageReader<Pointer<Press>>,
        mut drag_events: MessageReader<Pointer<Drag>>,
        camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
        player_query: Query<(), With<PlayerCoin>>,
        mut cursor_world: ResMut<CursorWorldPosition>,
    ) {
        for event in move_events.read() {
            if player_query.get(event.entity).is_ok() {
                cursor_world.0 = event.hit.position.map(|position| position.truncate());
            }
        }
        for event in over_events.read() {
            if player_query.get(event.entity).is_ok() {
                cursor_world.0 = event.hit.position.map(|position| position.truncate());
            }
        }
        for event in press_events.read() {
            if player_query.get(event.entity).is_ok() {
                cursor_world.0 = event.hit.position.map(|position| position.truncate());
            }
        }
        for event in drag_events.read() {
            if player_query.get(event.entity).is_ok() {
                cursor_world.0 =
                    pointer_world_position(&camera_query, event.pointer_location.position);
            }
        }
    }

    pub fn update_player_hover_state(
        mut over_events: MessageReader<Pointer<Over>>,
        mut out_events: MessageReader<Pointer<Out>>,
        mut player_query: Query<(Entity, &mut PlayerCoinState, &Velocity), With<PlayerCoin>>,
    ) {
        for (player_entity, mut player_state, velocity) in player_query.iter_mut() {
            if !matches!(
                player_state.state(),
                PlayerCoinBehaviorStatus::Idle | PlayerCoinBehaviorStatus::Aiming
            ) {
                continue;
            }

            for event in over_events.read() {
                if event.entity.eq(&player_entity) && player_state.is_idle() {
                    player_state.set_state(PlayerCoinBehaviorStatus::Aiming);
                }
            }
            for event in out_events.read() {
                if event.entity.eq(&player_entity) && player_state.is_aiming() {
                    player_state.set_state(PlayerCoinBehaviorStatus::Idle);
                }
            }
        }
    }

    pub fn start_player_charge_from_pointer(
        mut press_events: MessageReader<Pointer<Press>>,
        mut player_query: Query<(&mut PlayerCoinState, &Velocity), With<PlayerCoin>>,
    ) {
        for event in press_events.read() {
            if event.button != PointerButton::Primary {
                continue;
            }

            match player_query.get_mut(event.entity) {
                Ok((mut player_state, velocity)) => {
                    if !matches!(
                        player_state.state(),
                        PlayerCoinBehaviorStatus::Idle | PlayerCoinBehaviorStatus::Aiming
                    ) {
                        continue;
                    }

                    player_state.set_state(PlayerCoinBehaviorStatus::Charging {
                        eject_vector: Vec2::ZERO,
                    });
                }
                Err(_) => {}
            }
        }
    }

    pub fn handle_player_pointer_drag(
        config: Res<GameConfig>,
        mut drag_events: MessageReader<Pointer<Drag>>,
        camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
        mut player_query: Query<(&mut PlayerCoinState, &Transform), With<PlayerCoin>>,
    ) {
        for event in drag_events.read() {
            if event.button != PointerButton::Primary {
                continue;
            }

            match player_query.get_mut(event.entity) {
                Ok((mut player_state, player_transform)) => {
                    if !player_state.is_charging() {
                        continue;
                    }

                    let player_position = player_transform.translation.truncate();
                    let Some(cursor) =
                        pointer_world_position(&camera_query, event.pointer_location.position)
                    else {
                        continue;
                    };

                    player_state.set_state(PlayerCoinBehaviorStatus::Charging {
                        eject_vector: (player_position - cursor)
                            .clamp_length_max(config.player.max_eject_distance),
                    });
                }
                Err(_) => {}
            }
        }
    }

    pub fn finish_player_charge_from_pointer(
        config: Res<GameConfig>,
        mut release_events: MessageReader<Pointer<Release>>,
        mut drag_end_events: MessageReader<Pointer<DragEnd>>,
        mut player_query: Query<(&mut PlayerCoinState, &mut Velocity), With<PlayerCoin>>,
    ) {
        let mut released_at = HashSet::new();

        let _ = release_events
            .read()
            .map(|event| (event.button, event.entity))
            .chain(
                drag_end_events
                    .read()
                    .map(|event| (event.button, event.entity)),
            )
            .filter(|(button, _)| button.eq(&PointerButton::Primary))
            .for_each(|(_, entity)| {
                released_at.insert(entity);
            });

        if released_at.is_empty() {
            return;
        }

        for entity in released_at {
            match player_query.get_mut(entity) {
                Ok((mut player_state, mut velocity)) => {
                    if !player_state.is_charging() {
                        continue;
                    }

                    let eject_vector = player_state.eject_vector();
                    if eject_vector.length() > config.player.min_launch_distance {
                        let charge_ratio = eject_vector.length() / config.player.max_eject_distance;
                        let planar_velocity = charge_ratio
                            * config.player.max_planar_speed
                            * eject_vector.normalize_or_zero();
                        let vertical_velocity = config.player.min_vertical_speed
                            + charge_ratio
                                * (config.player.max_vertical_speed
                                    - config.player.min_vertical_speed);

                        velocity.x = planar_velocity.x;
                        velocity.y = planar_velocity.y;
                        velocity.z = vertical_velocity;
                        player_state.set_state(PlayerCoinBehaviorStatus::new_upspring());
                    } else {
                        player_state.set_state(PlayerCoinBehaviorStatus::Aiming);
                    }
                }
                Err(_) => {}
            }
        }
    }

    fn pointer_world_position(
        camera_query: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
        position: Vec2,
    ) -> Option<Vec2> {
        camera_query
            .iter()
            .filter(|(camera, _)| camera.is_active)
            .max_by_key(|(camera, _)| camera.order)
            .and_then(|(camera, camera_transform)| {
                camera.viewport_to_world_2d(camera_transform, position).ok()
            })
    }

    pub fn update_aiming_marker(
        cursor_world: Res<CursorWorldPosition>,
        player_query: Query<&PlayerCoinState, With<PlayerCoin>>,
        mut marker_query: Query<(&mut Transform, &mut Visibility), With<PointerMarker>>,
    ) {
        let Ok((mut transform, mut visibility)) = marker_query.single_mut() else {
            return;
        };

        for player_state in player_query.iter() {
            if player_state.is_aiming() || player_state.is_charging() {
                if let Some(cursor) = cursor_world.0 {
                    transform.translation =
                        cursor.extend(SceneLayer::GizmoAimingMarker.get_layer_base_z());
                    *visibility = Visibility::Visible;
                    return;
                }
            }

            *visibility = Visibility::Hidden;
        }
    }

    pub fn update_player_visuals(
        config: Res<GameConfig>,
        mut player_query: Query<(&PlayerCoinState, &mut Transform), With<PlayerCoin>>,
    ) {
        for (player_state, mut transform) in player_query.iter_mut() {
            let scale = 1.0 + player_state.sim_z() * config.player.height_scale_factor;

            transform.scale = Vec3::splat(scale);
        }
    }

    pub fn draw_arena_and_aim(
        config: Res<GameConfig>,
        mut gizmos: Gizmos,
        player_query: Query<(&PlayerCoinState, &Transform), With<PlayerCoin>>,
    ) {
        for (player_state, player_transform) in player_query.iter() {
            let player_position = player_transform.translation.truncate();
            gizmos.circle_2d(
                player_position,
                config.visuals.player_radius + config.player.aim_ring_padding,
                config.player.aim_ring_color(),
            );

            let eject_vector = player_state.eject_vector();
            if player_state.is_charging() && eject_vector != Vec2::ZERO {
                let cursor_position = player_position + eject_vector;
                let launch_target = player_position - eject_vector;

                gizmos.line_2d(
                    player_position,
                    cursor_position,
                    config.player.charge_line_color(),
                );
                gizmos.line_2d(
                    player_position,
                    launch_target,
                    config.player.launch_line_color(),
                );
                gizmos.circle_2d(
                    launch_target,
                    config.player.launch_marker_radius,
                    config.player.launch_line_color(),
                );
            }
        }
    }
}

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CursorWorldPosition>()
            .add_systems(
                OnEnter(GameStatus::Loading),
                (
                    controller::setup_player,
                    controller::set_player_coin_state_idle,
                )
                    .chain()
                    .in_set(GameLoadingSet::BuildScene),
            )
            .add_systems(
                Update,
                track_pointer_world_position
                    .in_set(GameplaySet::PlayerInput)
                    .run_if(in_state(GameStatus::InGame).and(player_input_allowed)),
            )
            .add_systems(
                Update,
                (
                    update_player_hover_state,
                    start_player_charge_from_pointer,
                    handle_player_pointer_drag,
                    finish_player_charge_from_pointer,
                )
                    .after(track_pointer_world_position)
                    .in_set(GameplaySet::PlayerInput)
                    .run_if(in_state(GameStatus::InGame).and(player_input_allowed)),
            )
            .add_systems(
                Update,
                (
                    update_aiming_marker,
                    draw_arena_and_aim,
                    update_player_visuals,
                )
                    .in_set(GameplaySet::Visual),
            )
            .add_systems(
                Update,
                controller::handle_death
                    .in_set(GameplaySet::Respawn)
                    .run_if(in_state(GameStatus::InGame)),
            );
    }
}
