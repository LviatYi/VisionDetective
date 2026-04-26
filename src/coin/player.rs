use bevy::prelude::Component;

#[derive(Component)]
pub struct PlayerCoin;

pub mod controller {
    use crate::coin::player::PlayerCoin;
    use crate::physics::{ARENA_HALF_HEIGHT, ARENA_HALF_WIDTH, Velocity};
    use crate::{CursorWorldPosition, EJECT_POWER, MAX_EJECT_DISTANCE, MAX_SPEED, PLAYER_RADIUS};
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
        pub building_up: bool,
        pub aiming: bool,
        pub eject_vector: Vec2,
    }

    pub fn handle_player_eject_input(
        mouse_input: Res<ButtonInput<MouseButton>>,
        cursor_world: Res<CursorWorldPosition>,
        mut input_state: ResMut<EjectInputState>,
        mut player_query: Query<(&Transform, &mut Velocity), With<PlayerCoin>>,
    ) {
        let Ok((player_transform, mut velocity)) = player_query.single_mut() else {
            return;
        };

        let player_position = player_transform.translation.truncate();
        let cursor_position = cursor_world.0;
        input_state.aiming = cursor_position
            .map(|cursor| cursor.distance(player_position) <= PLAYER_RADIUS)
            .map(|in_range| in_range && velocity.length() <= 0.0)
            .unwrap_or(false);

        if mouse_input.just_pressed(MouseButton::Left) && input_state.aiming {
            input_state.building_up = true;
        }

        if input_state.building_up {
            if mouse_input.pressed(MouseButton::Left) {
                if let Some(cursor) = cursor_position {
                    input_state.eject_vector =
                        (player_position - cursor).clamp_length_max(MAX_EJECT_DISTANCE);
                }
            } else {
                if input_state.eject_vector.length() > 6.0 {
                    **velocity =
                        (input_state.eject_vector * EJECT_POWER).clamp_length_max(MAX_SPEED);
                }
                input_state.building_up = false;
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

        if drag_state.aiming || drag_state.building_up {
            if let Some(cursor) = cursor_world.0 {
                transform.translation = cursor.extend(4.0);
                *visibility = Visibility::Visible;
                return;
            }
        }

        *visibility = Visibility::Hidden;
    }

    pub fn update_player_visuals(
        mut player_query: Query<(&Velocity, &mut Transform), With<PlayerCoin>>,
    ) {
        let Ok((velocity, mut transform)) = player_query.single_mut() else {
            return;
        };

        let speed_ratio = velocity.length() / MAX_SPEED;
        let scale = 1.0 + speed_ratio;

        transform.scale = Vec3::splat(scale);
        transform.translation.z = 2.0 + speed_ratio * 8.0;
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

        if drag_state.building_up && drag_state.eject_vector != Vec2::ZERO {
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
