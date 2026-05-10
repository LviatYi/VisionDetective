use crate::coin::player::controller::{
    CursorWorldPosition, PlayerCoinState, draw_arena_and_aim, finish_player_charge_from_pointer,
    handle_player_pointer_drag, start_player_charge_from_pointer, track_pointer_world_position,
    update_aiming_marker, update_player_hover_state, update_player_visuals,
};
use crate::game_view::GameState;
use crate::input::player_input_allowed;
use bevy::prelude::*;

#[derive(Component, Default)]
pub struct PlayerCoin {
    pub sim_z: f32,
    pub ground_contact_count: u8,
}

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
        Assets, Camera, Camera2d, Circle, ColorMaterial, Commands, Component, Gizmos,
        GlobalTransform, Mesh, Mesh2d, MeshMaterial2d, MessageReader, Pickable, Query, Res, ResMut,
        Resource, Transform, Visibility, With,
    };
    use std::ops::Deref;

    #[derive(Component)]
    pub struct PointerMarker;

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub enum EPlayerCoinState {
        #[default]
        Idle,
        Aiming,
        Charging {
            eject_vector: Vec2,
        },
        Ejecting,
    }

    #[derive(Resource, Default)]
    pub struct PlayerCoinState {
        state: EPlayerCoinState,

        pub last: Option<EPlayerCoinState>,
    }

    impl PlayerCoinState {
        pub fn state(&self) {
            self.state;
        }

        pub fn set_state(&mut self, state: EPlayerCoinState) {
            self.last = Some(self.state);
            self.state = state;
        }
    }

    impl Deref for PlayerCoinState {
        type Target = EPlayerCoinState;

        fn deref(&self) -> &Self::Target {
            &self.state
        }
    }

    impl PlayerCoinState {
        pub fn is_idle(&self) -> bool {
            matches!(self.state, EPlayerCoinState::Idle)
        }

        pub fn is_stop(&self) -> bool {
            !matches!(self.state, EPlayerCoinState::Ejecting)
        }

        pub fn is_aiming(&self) -> bool {
            matches!(self.state, EPlayerCoinState::Aiming)
        }

        pub fn is_charging(&self) -> bool {
            matches!(self.state, EPlayerCoinState::Charging { .. })
        }

        pub fn just_ejected(&self) -> bool {
            matches!(self.state, EPlayerCoinState::Idle)
                && self
                    .last
                    .is_some_and(|s| matches!(s, EPlayerCoinState::Ejecting))
        }

        pub fn eject_vector(&self) -> Vec2 {
            match self.state {
                EPlayerCoinState::Charging { eject_vector } => eject_vector,
                _ => Vec2::ZERO,
            }
        }
    }

    #[derive(Resource, Default)]
    pub struct CursorWorldPosition(pub Option<Vec2>);

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
                SceneLayer::PlayerCoin.get_layer_base_z() + config.visuals.player_z,
            )),
            PlayerCoin::default(),
            Velocity::default(),
            Pickable::default(),
            crate::GameView,
        ));

        commands.spawn((
            Mesh2d(meshes.add(Circle::new(config.visuals.pointer_radius))),
            MeshMaterial2d(materials.add(config.visuals.pointer_color())),
            Transform::from_translation(Vec3::new(0.0, 0.0, config.visuals.pointer_z)),
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
        mut player_state: ResMut<PlayerCoinState>,
        player_query: Query<&Velocity, With<PlayerCoin>>,
    ) {
        let Ok(velocity) = player_query.single() else {
            return;
        };
        if !player_state.is_idle() && !player_state.is_aiming() {
            return;
        }

        for event in over_events.read() {
            if event.hit.position.is_some() && velocity.length_squared() <= 0.0 {
                player_state.set_state(EPlayerCoinState::Aiming);
            }
        }
        for _ in out_events.read() {
            if player_state.is_aiming() {
                player_state.set_state(EPlayerCoinState::Idle);
            }
        }
    }

    pub fn start_player_charge_from_pointer(
        mut press_events: MessageReader<Pointer<Press>>,
        mut player_state: ResMut<PlayerCoinState>,
        player_query: Query<&Velocity, With<PlayerCoin>>,
    ) {
        let Ok(velocity) = player_query.single() else {
            return;
        };
        if velocity.length_squared() > 0.0 || matches!(**player_state, EPlayerCoinState::Ejecting) {
            return;
        }

        for event in press_events.read() {
            if event.button == PointerButton::Primary && player_query.get(event.entity).is_ok() {
                player_state.set_state(EPlayerCoinState::Charging {
                    eject_vector: Vec2::ZERO,
                });
            }
        }
    }

    pub fn handle_player_pointer_drag(
        config: Res<GameConfig>,
        mut drag_events: MessageReader<Pointer<Drag>>,
        camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
        mut player_state: ResMut<PlayerCoinState>,
        player_query: Query<&Transform, With<PlayerCoin>>,
    ) {
        if !player_state.is_charging() {
            return;
        }

        let Ok(player_transform) = player_query.single() else {
            return;
        };
        let player_position = player_transform.translation.truncate();

        for event in drag_events.read() {
            if event.button != PointerButton::Primary || player_query.get(event.entity).is_err() {
                continue;
            }
            let Some(cursor) =
                pointer_world_position(&camera_query, event.pointer_location.position)
            else {
                continue;
            };
            player_state.set_state(EPlayerCoinState::Charging {
                eject_vector: (player_position - cursor)
                    .clamp_length_max(config.player.max_eject_distance),
            });
        }
    }

    pub fn finish_player_charge_from_pointer(
        config: Res<GameConfig>,
        mut release_events: MessageReader<Pointer<Release>>,
        mut drag_end_events: MessageReader<Pointer<DragEnd>>,
        mut player_state: ResMut<PlayerCoinState>,
        mut player_query: Query<(&mut PlayerCoin, &mut Velocity), With<PlayerCoin>>,
    ) {
        if !player_state.is_charging() {
            return;
        }

        let mut released = false;
        for event in release_events.read() {
            released |=
                event.button == PointerButton::Primary && player_query.get(event.entity).is_ok();
        }
        for event in drag_end_events.read() {
            released |=
                event.button == PointerButton::Primary && player_query.get(event.entity).is_ok();
        }
        if !released {
            return;
        }

        let Ok((mut player, mut velocity)) = player_query.single_mut() else {
            return;
        };
        let eject_vector = player_state.eject_vector();
        if eject_vector.length() > config.player.min_launch_distance {
            let charge_ratio = eject_vector.length() / config.player.max_eject_distance;
            let planar_velocity =
                charge_ratio * config.player.max_planar_speed * eject_vector.normalize_or_zero();
            let vertical_velocity = config.player.min_vertical_speed
                + charge_ratio
                    * (config.player.max_vertical_speed - config.player.min_vertical_speed);

            velocity.x = planar_velocity.x;
            velocity.y = planar_velocity.y;
            velocity.z = vertical_velocity;
            player.sim_z = 0.0;
            player.ground_contact_count = 0;
            player_state.set_state(EPlayerCoinState::Ejecting);
        } else {
            player_state.set_state(EPlayerCoinState::Aiming);
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
        config: Res<GameConfig>,
        cursor_world: Res<CursorWorldPosition>,
        player_state: Res<PlayerCoinState>,
        mut marker_query: Query<(&mut Transform, &mut Visibility), With<PointerMarker>>,
    ) {
        let Ok((mut transform, mut visibility)) = marker_query.single_mut() else {
            return;
        };

        if player_state.is_aiming() || player_state.is_charging() {
            if let Some(cursor) = cursor_world.0 {
                transform.translation = cursor.extend(config.visuals.marker_z);
                *visibility = Visibility::Visible;
                return;
            }
        }

        *visibility = Visibility::Hidden;
    }

    pub fn update_player_visuals(
        config: Res<GameConfig>,
        mut player_query: Query<(&PlayerCoin, &mut Transform), With<PlayerCoin>>,
    ) {
        let Ok((player, mut transform)) = player_query.single_mut() else {
            return;
        };

        let scale = 1.0 + player.sim_z * config.player.height_scale_factor;

        transform.scale = Vec3::splat(scale);
    }

    pub fn draw_arena_and_aim(
        config: Res<GameConfig>,
        mut gizmos: Gizmos,
        player_state: Res<PlayerCoinState>,
        player_query: Query<&Transform, With<PlayerCoin>>,
    ) {
        let Ok(player_transform) = player_query.single() else {
            return;
        };

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

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CursorWorldPosition>()
            .init_resource::<PlayerCoinState>()
            .add_systems(OnEnter(GameState::Loading), controller::setup_player)
            .add_systems(
                Update,
                track_pointer_world_position
                    .run_if(in_state(GameState::InGame).and(player_input_allowed)),
            )
            .add_systems(
                Update,
                (
                    update_player_hover_state,
                    start_player_charge_from_pointer,
                    handle_player_pointer_drag,
                    finish_player_charge_from_pointer,
                    update_aiming_marker,
                    draw_arena_and_aim,
                )
                    .after(track_pointer_world_position)
                    .run_if(in_state(GameState::InGame).and(player_input_allowed)),
            )
            .add_systems(
                Update,
                update_player_visuals.run_if(in_state(GameState::InGame)),
            );
    }
}
