use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::PlayerCoinState;
use crate::game_view::GameplaySet;
use crate::input::GameplayInputBlocker;
use crate::physics::{Velocity, move_player_coin_transform};
use bevy::prelude::*;

pub struct CameraControlPlugin;

#[derive(Component)]
pub struct GameCamera;

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
enum CameraControlPhase {
    #[default]
    HoldOn,
    /// record the velocity.
    Pursue(Vec2),
    /// record a player to camera offset.
    Follow(Vec2),
    /// record the velocity.
    Focus(Vec2),
}

impl Plugin for CameraControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraControlPhase>().add_systems(
            Update,
            update_game_camera
                .after(move_player_coin_transform)
                .in_set(GameplaySet::PlayerPhysics),
        );
    }
}

fn update_game_camera(
    time: Res<Time>,
    player_state: Res<PlayerCoinState>,
    player_query: Query<(&Transform, &Velocity), (With<PlayerCoin>, Without<GameCamera>)>,
    mut camera_query: Query<(&mut Transform, &Projection), (With<GameCamera>, Without<PlayerCoin>)>,
    mut phase: ResMut<CameraControlPhase>,
    mut input_blocker: ResMut<GameplayInputBlocker>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    let Ok((player_transform, player_velocity)) = player_query.single() else {
        return;
    };
    let Ok((mut camera_transform, projection)) = camera_query.single_mut() else {
        return;
    };
    let Projection::Orthographic(orthographic) = projection else {
        return;
    };

    let player_position = player_transform.translation.truncate();
    let player_velocity = player_velocity.truncate();
    let camera_position = camera_transform.translation.truncate();
    let viewport_half_size = orthographic.area.size() * 0.5;
    let edge_line = viewport_half_size * (1.0 - VIEWPORT_EDGE_RATIO * 2.0);
    let relative_position = player_position - camera_position;

    if player_state.is_stop() || player_velocity.length_squared() < PLAYER_MOVING_EPSILON_SQUARED {
        focus_camera_on_player(dt, player_position, &mut camera_transform, &mut phase);
        update_camera_focus_input_blocker(&phase, &mut input_blocker);
        return;
    }

    if let CameraControlPhase::Follow(player_to_camera_offset) = *phase {
        follow_player(
            player_to_camera_offset,
            player_transform,
            &mut camera_transform,
            &mut phase,
        );
        update_camera_focus_input_blocker(&phase, &mut input_blocker);
        return;
    }

    if touches_viewport_edge(relative_position, edge_line) {
        pursue_player(
            dt,
            player_velocity,
            player_transform,
            &mut camera_transform,
            &mut phase,
        );
    } else {
        hold_camera(dt, &mut phase);
    }
    update_camera_focus_input_blocker(&phase, &mut input_blocker);
}

fn focus_camera_on_player(
    dt: f32,
    player_position: Vec2,
    camera_transform: &mut Transform,
    phase: &mut CameraControlPhase,
) {
    let previous_velocity = phase_velocity(*phase);

    let current = camera_transform.translation.truncate();
    let alpha = smooth_alpha(FOCUS_SMOOTHNESS, dt);
    let next = current.lerp(player_position, alpha);
    let velocity = (next - current) / dt;
    camera_transform.translation.x = next.x;
    camera_transform.translation.y = next.y;

    if (player_position - next).length_squared() <= FOCUS_REST_DISTANCE_SQUARED {
        camera_transform.translation.x = player_position.x;
        camera_transform.translation.y = player_position.y;
        *phase = CameraControlPhase::HoldOn;
    } else {
        *phase = CameraControlPhase::Focus(if velocity.is_finite() {
            velocity
        } else {
            previous_velocity
        });
    }
}

fn pursue_player(
    dt: f32,
    player_velocity: Vec2,
    player_transform: &Transform,
    camera_transform: &mut Transform,
    phase: &mut CameraControlPhase,
) {
    let speed = player_velocity.length();
    if speed <= PLAYER_MOVING_EPSILON {
        return;
    }

    let current_velocity = phase_velocity(*phase);
    let delta_velocity = player_velocity - current_velocity;
    let acceleration = speed / PURSUE_CATCH_UP_DISTANCE.max(1.0);
    let max_velocity_delta = delta_velocity.length() * acceleration * dt;
    let next_velocity = approach_vec2(
        current_velocity,
        player_velocity,
        max_velocity_delta.max(MIN_PURSUIT_VELOCITY_DELTA),
    );

    camera_transform.translation.x += next_velocity.x * dt;
    camera_transform.translation.y += next_velocity.y * dt;

    if (player_velocity - next_velocity).length_squared() <= FOLLOW_VELOCITY_EPSILON_SQUARED {
        *phase = CameraControlPhase::Follow(
            camera_transform.translation.truncate() - player_transform.translation.truncate(),
        );
    } else {
        *phase = CameraControlPhase::Pursue(next_velocity);
    }
}

fn follow_player(
    player_to_camera_offset: Vec2,
    player_transform: &Transform,
    camera_transform: &mut Transform,
    phase: &mut CameraControlPhase,
) {
    let camera_position = player_transform.translation.truncate() + player_to_camera_offset;
    camera_transform.translation.x = camera_position.x;
    camera_transform.translation.y = camera_position.y;
    *phase = CameraControlPhase::Follow(player_to_camera_offset);
}

fn hold_camera(dt: f32, phase: &mut CameraControlPhase) {
    let velocity = phase_velocity(*phase) * (1.0 - smooth_alpha(HOLD_VELOCITY_DAMPING, dt));
    if velocity.length_squared() <= HOLD_REST_VELOCITY_SQUARED {
        *phase = CameraControlPhase::HoldOn;
    } else {
        *phase = CameraControlPhase::Pursue(velocity);
    }
}

fn touches_viewport_edge(relative_position: Vec2, edge_line: Vec2) -> bool {
    relative_position.x.abs() >= edge_line.x || relative_position.y.abs() >= edge_line.y
}

fn approach_vec2(current: Vec2, target: Vec2, max_delta: f32) -> Vec2 {
    let delta = target - current;
    let distance = delta.length();
    if distance <= max_delta || distance <= f32::EPSILON {
        target
    } else {
        current + delta / distance * max_delta
    }
}

fn smooth_alpha(smoothness: f32, dt: f32) -> f32 {
    1.0 - (-smoothness * dt).exp()
}

fn phase_velocity(phase: CameraControlPhase) -> Vec2 {
    match phase {
        CameraControlPhase::HoldOn | CameraControlPhase::Follow(_) => Vec2::ZERO,
        CameraControlPhase::Pursue(velocity) | CameraControlPhase::Focus(velocity) => velocity,
    }
}

fn update_camera_focus_input_blocker(
    phase: &CameraControlPhase,
    input_blocker: &mut GameplayInputBlocker,
) {
    if matches!(phase, CameraControlPhase::Focus(_)) {
        input_blocker.block(CAMERA_FOCUS_INPUT_BLOCKER);
    } else {
        input_blocker.unblock(CAMERA_FOCUS_INPUT_BLOCKER);
    }
}

const VIEWPORT_EDGE_RATIO: f32 = 0.3;
const PURSUE_CATCH_UP_DISTANCE: f32 = 240.0;
const FOCUS_SMOOTHNESS: f32 = 4.8;
const HOLD_VELOCITY_DAMPING: f32 = 7.0;
const PLAYER_MOVING_EPSILON: f32 = 0.1;
const PLAYER_MOVING_EPSILON_SQUARED: f32 = PLAYER_MOVING_EPSILON * PLAYER_MOVING_EPSILON;
const FOLLOW_VELOCITY_EPSILON: f32 = 6.0;
const FOLLOW_VELOCITY_EPSILON_SQUARED: f32 = FOLLOW_VELOCITY_EPSILON * FOLLOW_VELOCITY_EPSILON;
const FOCUS_REST_DISTANCE: f32 = 0.5;
const FOCUS_REST_DISTANCE_SQUARED: f32 = FOCUS_REST_DISTANCE * FOCUS_REST_DISTANCE;
const HOLD_REST_VELOCITY: f32 = 0.5;
const HOLD_REST_VELOCITY_SQUARED: f32 = HOLD_REST_VELOCITY * HOLD_REST_VELOCITY;
const MIN_PURSUIT_VELOCITY_DELTA: f32 = 0.01;
const CAMERA_FOCUS_INPUT_BLOCKER: &str = "camera_focusing";
