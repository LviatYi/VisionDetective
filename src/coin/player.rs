use crate::physics::Velocity;
use crate::{CursorWorldPosition, MAX_PULL_DISTANCE, MAX_SPEED, PLAYER_RADIUS, SHOT_POWER};
use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::prelude::{Component, MouseButton, Query, Res, ResMut, Resource, Transform, With};

#[derive(Component)]
pub struct PlayerCoin;

#[derive(Component)]
pub struct PointerMarker;

#[derive(Resource, Default)]
pub struct LaunchDragState {
    pub active: bool,
    pub hover_player: bool,
    pub pull_vector: Vec2,
}


pub fn handle_player_drag(
    mouse_input: Res<ButtonInput<MouseButton>>,
    cursor_world: Res<CursorWorldPosition>,
    mut drag_state: ResMut<LaunchDragState>,
    mut player_query: Query<(&Transform, &mut Velocity), With<PlayerCoin>>,
) {
    let Ok((player_transform, mut velocity)) = player_query.single_mut() else {
        return;
    };

    let player_position = player_transform.translation.truncate();
    let cursor_position = cursor_world.0;
    drag_state.hover_player = cursor_position
        .map(|cursor| cursor.distance(player_position) <= PLAYER_RADIUS)
        .unwrap_or(false);

    if mouse_input.just_pressed(MouseButton::Left) && drag_state.hover_player {
        drag_state.active = true;
    }

    if drag_state.active {
        if mouse_input.pressed(MouseButton::Left) {
            if let Some(cursor) = cursor_position {
                drag_state.pull_vector =
                    (cursor - player_position).clamp_length_max(MAX_PULL_DISTANCE);
            }
        } else {
            if drag_state.pull_vector.length() > 6.0 {
                **velocity = (-drag_state.pull_vector * SHOT_POWER).clamp_length_max(MAX_SPEED);
            }
            drag_state.active = false;
            drag_state.pull_vector = Vec2::ZERO;
        }
    } else {
        drag_state.pull_vector = Vec2::ZERO;
    }
}