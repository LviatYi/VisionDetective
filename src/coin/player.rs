use crate::AppScreen;
use crate::coin::player::controller::{
    CursorWorldPosition, EjectInputState, draw_arena_and_aim, handle_player_eject_input,
    track_cursor_world_position, update_aiming_marker, update_player_visuals,
};
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
    use crate::editor::{EditorInteractionState, cursor_is_over_scene};
    use crate::physics::Velocity;
    use bevy::input::ButtonInput;
    use bevy::math::{Vec2, Vec3};
    use bevy::prelude::{
        Assets, Camera, Camera2d, Circle, ColorMaterial, Commands, Component, Gizmos,
        GlobalTransform, Mesh, Mesh2d, MeshMaterial2d, MouseButton, Query, Res, ResMut, Resource,
        Transform, Visibility, With,
    };
    use bevy::window::{PrimaryWindow, Window};

    #[derive(Component)]
    pub struct PointerMarker;

    #[derive(Resource, Default)]
    pub struct EjectInputState {
        pub aiming: bool,
        pub charging: bool,
        pub eject_vector: Vec2,
    }

    #[derive(Resource, Default)]
    pub struct CursorWorldPosition(pub Option<Vec2>);

    pub fn setup_player(
        mut commands: Commands,
        config: Res<GameConfig>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<ColorMaterial>>,
    ) {
        commands.spawn((
            Mesh2d(meshes.add(Circle::new(config.visuals.player_radius))),
            MeshMaterial2d(materials.add(config.visuals.player_color())),
            Transform::from_translation(Vec3::new(0.0, 0.0, config.visuals.player_z)),
            PlayerCoin::default(),
            Velocity::default(),
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

    pub fn track_cursor_world_position(
        window_query: Query<&Window, With<PrimaryWindow>>,
        camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
        mut cursor_world: ResMut<CursorWorldPosition>,
    ) {
        let (Ok(window), Ok((camera, camera_transform))) =
            (window_query.single(), camera_query.single())
        else {
            return;
        };

        cursor_world.0 = window
            .cursor_position()
            .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok());
    }

    pub fn handle_player_eject_input(
        config: Res<GameConfig>,
        mouse_input: Res<ButtonInput<MouseButton>>,
        cursor_world: Res<CursorWorldPosition>,
        window_query: Query<&Window, With<PrimaryWindow>>,
        editor_state: Option<Res<EditorInteractionState>>,
        mut input_state: ResMut<EjectInputState>,
        mut player_query: Query<(&Transform, &mut PlayerCoin, &mut Velocity), With<PlayerCoin>>,
    ) {
        let Ok((player_transform, mut player, mut velocity)) = player_query.single_mut() else {
            return;
        };

        let player_position = player_transform.translation.truncate();
        let cursor_position = cursor_world.0;
        let pointer_captured = editor_state
            .as_ref()
            .and_then(|state| {
                window_query
                    .single()
                    .ok()
                    .and_then(|window| window.cursor_position().map(|cursor| (window, cursor)))
                    .map(|(window, cursor)| {
                        !cursor_is_over_scene(window, cursor) || state.captures_pointer()
                    })
            })
            .unwrap_or(false);

        if pointer_captured {
            input_state.aiming = false;
            if !mouse_input.pressed(MouseButton::Left) {
                input_state.charging = false;
                input_state.eject_vector = Vec2::ZERO;
            }
            return;
        }

        input_state.aiming = cursor_position
            .map(|cursor| cursor.distance(player_position) <= config.visuals.player_radius)
            .map(|in_range| in_range && velocity.length_squared() <= 0.0)
            .unwrap_or(false);

        if mouse_input.just_pressed(MouseButton::Left) && input_state.aiming {
            input_state.charging = true;
        }

        if input_state.charging {
            if mouse_input.pressed(MouseButton::Left) {
                if let Some(cursor) = cursor_position {
                    input_state.eject_vector = (player_position - cursor)
                        .clamp_length_max(config.player.max_eject_distance);
                }
            } else {
                // eject the player coin
                if input_state.eject_vector.length() > config.player.min_launch_distance {
                    let charge_ratio =
                        input_state.eject_vector.length() / config.player.max_eject_distance;
                    let planar_velocity = charge_ratio
                        * config.player.max_planar_speed
                        * input_state.eject_vector.normalize_or_zero();
                    let vertical_velocity = config.player.min_vertical_speed
                        + charge_ratio
                            * (config.player.max_vertical_speed - config.player.min_vertical_speed);

                    velocity.x = planar_velocity.x;
                    velocity.y = planar_velocity.y;
                    velocity.z = vertical_velocity;
                    player.sim_z = 0.0;
                    player.ground_contact_count = 0;
                }
                input_state.charging = false;
                input_state.eject_vector = Vec2::ZERO;
            }
        } else {
            input_state.eject_vector = Vec2::ZERO;
        }
    }

    pub fn update_aiming_marker(
        config: Res<GameConfig>,
        cursor_world: Res<CursorWorldPosition>,
        drag_state: Res<EjectInputState>,
        mut marker_query: Query<(&mut Transform, &mut Visibility), With<PointerMarker>>,
    ) {
        let Ok((mut transform, mut visibility)) = marker_query.single_mut() else {
            return;
        };

        if drag_state.aiming || drag_state.charging {
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
        drag_state: Res<EjectInputState>,
        player_query: Query<&Transform, With<PlayerCoin>>,
    ) {
        gizmos.rect_2d(
            Vec2::ZERO,
            Vec2::new(
                config.physics.arena_half_width * 2.0,
                config.physics.arena_half_height * 2.0,
            ),
            config.player.arena_outline_color(),
        );

        let Ok(player_transform) = player_query.single() else {
            return;
        };

        let player_position = player_transform.translation.truncate();
        gizmos.circle_2d(
            player_position,
            config.visuals.player_radius + config.player.aim_ring_padding,
            config.player.aim_ring_color(),
        );

        if drag_state.charging && drag_state.eject_vector != Vec2::ZERO {
            let cursor_position = player_position + drag_state.eject_vector;
            let launch_target = player_position - drag_state.eject_vector;

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
            .init_resource::<EjectInputState>()
            .add_systems(OnEnter(AppScreen::Game), controller::setup_player)
            .add_systems(Update, track_cursor_world_position)
            .add_systems(
                Update,
                (
                    handle_player_eject_input,
                    update_player_visuals,
                    update_aiming_marker,
                    draw_arena_and_aim,
                )
                    .run_if(in_state(AppScreen::Game)),
            );
    }
}
