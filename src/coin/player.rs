use bevy::prelude::Component;

pub const MAX_EJECT_DISTANCE: f32 = 100.0;
pub const MAX_PLANAR_SPEED: f32 = 250.0;
pub const MAX_VERTICAL_SPEED: f32 = 4.0;
pub const MIN_VERTICAL_SPEED: f32 = 0.5;
pub const HEIGHT_SCALE_FACTOR: f32 = 0.75;

#[derive(Component, Default)]
pub struct PlayerCoin {
    pub sim_z: f32,
    pub ground_contact_count: u8,
}

pub mod controller {
    use crate::coin::player::{
        HEIGHT_SCALE_FACTOR, MAX_EJECT_DISTANCE, MAX_PLANAR_SPEED, MAX_VERTICAL_SPEED,
        MIN_VERTICAL_SPEED, PlayerCoin,
    };
    use crate::physics::{ARENA_HALF_HEIGHT, ARENA_HALF_WIDTH, Velocity};
    use crate::{CursorWorldPosition, PLAYER_RADIUS};
    use bevy::color::Color;
    use bevy::input::ButtonInput;
    use bevy::math::{Vec2, Vec3};
    use bevy::prelude::{
        Component, Gizmos, MouseButton, Query, Res, ResMut, Resource, Transform, Visibility, With,
    };

    #[derive(Component)]
    pub struct PointerMarker;

    #[derive(Resource, Default)]
    pub struct EjectInputState {
        pub aiming: bool,
        pub charging: bool,
        pub eject_vector: Vec2,
    }

    pub fn handle_player_eject_input(
        mouse_input: Res<ButtonInput<MouseButton>>,
        cursor_world: Res<CursorWorldPosition>,
        mut input_state: ResMut<EjectInputState>,
        mut player_query: Query<(&Transform, &mut PlayerCoin, &mut Velocity), With<PlayerCoin>>,
    ) {
        let Ok((player_transform, mut player, mut velocity)) = player_query.single_mut() else {
            return;
        };

        let player_position = player_transform.translation.truncate();
        let cursor_position = cursor_world.0;
        input_state.aiming = cursor_position
            .map(|cursor| cursor.distance(player_position) <= PLAYER_RADIUS)
            .map(|in_range| in_range && velocity.length_squared() <= 0.0)
            .unwrap_or(false);

        if mouse_input.just_pressed(MouseButton::Left) && input_state.aiming {
            input_state.charging = true;
        }

        if input_state.charging {
            if mouse_input.pressed(MouseButton::Left) {
                if let Some(cursor) = cursor_position {
                    input_state.eject_vector =
                        (player_position - cursor).clamp_length_max(MAX_EJECT_DISTANCE);
                }
            } else {
                // eject the player coin
                if input_state.eject_vector.length() > 6.0 {
                    let charge_ratio = input_state.eject_vector.length() / MAX_EJECT_DISTANCE;
                    let planar_velocity = charge_ratio
                        * MAX_PLANAR_SPEED
                        * input_state.eject_vector.normalize_or_zero();
                    let vertical_velocity = MIN_VERTICAL_SPEED
                        + charge_ratio * (MAX_VERTICAL_SPEED - MIN_VERTICAL_SPEED);

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
        cursor_world: Res<CursorWorldPosition>,
        drag_state: Res<EjectInputState>,
        mut marker_query: Query<(&mut Transform, &mut Visibility), With<PointerMarker>>,
    ) {
        let Ok((mut transform, mut visibility)) = marker_query.single_mut() else {
            return;
        };

        if drag_state.aiming || drag_state.charging {
            if let Some(cursor) = cursor_world.0 {
                transform.translation = cursor.extend(4.0);
                *visibility = Visibility::Visible;
                return;
            }
        }

        *visibility = Visibility::Hidden;
    }

    pub fn update_player_visuals(
        mut player_query: Query<(&PlayerCoin, &mut Transform), With<PlayerCoin>>,
    ) {
        let Ok((player, mut transform)) = player_query.single_mut() else {
            return;
        };

        let scale = 1.0 + player.sim_z * HEIGHT_SCALE_FACTOR;

        transform.scale = Vec3::splat(scale);
    }

    pub fn draw_arena_and_aim(
        mut gizmos: Gizmos,
        drag_state: Res<EjectInputState>,
        player_query: Query<&Transform, With<PlayerCoin>>,
    ) {
        gizmos.rect_2d(
            Vec2::ZERO,
            Vec2::new(ARENA_HALF_WIDTH * 2.0, ARENA_HALF_HEIGHT * 2.0),
            Color::srgb(0.22, 0.28, 0.31),
        );

        let Ok(player_transform) = player_query.single() else {
            return;
        };

        let player_position = player_transform.translation.truncate();
        gizmos.circle_2d(
            player_position,
            PLAYER_RADIUS + 6.0,
            Color::srgba(1.0, 1.0, 1.0, 0.12),
        );

        if drag_state.charging && drag_state.eject_vector != Vec2::ZERO {
            let cursor_position = player_position + drag_state.eject_vector;
            let launch_target = player_position - drag_state.eject_vector;

            gizmos.line_2d(
                player_position,
                cursor_position,
                Color::srgb(0.98, 0.43, 0.29),
            );
            gizmos.line_2d(
                player_position,
                launch_target,
                Color::srgb(0.26, 0.87, 0.71),
            );
            gizmos.circle_2d(launch_target, 12.0, Color::srgb(0.26, 0.87, 0.71));
        }
    }
}
