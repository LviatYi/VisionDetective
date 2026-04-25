use bevy::math::Vec2;
use bevy::prelude::{Component, Deref, DerefMut, Query, Res, Time, Transform, With};
use crate::PLAYER_RADIUS;

pub const ARENA_HALF_WIDTH: f32 = 500.0;
pub const ARENA_HALF_HEIGHT: f32 = 280.0;
pub const BOUNCE_FACTOR: f32 = 0.72;
pub const FRICTION: f32 = 1.7;
pub const STOP_SPEED: f32 = 8.0;

#[derive(Component, Deref, DerefMut, Default)]
pub struct Velocity(Vec2);

pub fn move_transform(
    time: Res<Time>,
    mut player_query: Query<(&mut Transform, &mut Velocity)>,
) {
    let Ok((mut transform, mut velocity)) = player_query.single_mut() else {
        return;
    };

    let dt = time.delta_secs();
    transform.translation += velocity.extend(0.0) * dt;

    //region TODO_LviatYi: 根据地形进行碰撞检测和响应，而非简单的盒模型 
    let min_x = -ARENA_HALF_WIDTH + PLAYER_RADIUS;
    let max_x = ARENA_HALF_WIDTH - PLAYER_RADIUS;
    let min_y = -ARENA_HALF_HEIGHT + PLAYER_RADIUS;
    let max_y = ARENA_HALF_HEIGHT - PLAYER_RADIUS;

    if transform.translation.x < min_x {
        transform.translation.x = min_x;
        velocity.x = velocity.x.abs() * BOUNCE_FACTOR;
    } else if transform.translation.x > max_x {
        transform.translation.x = max_x;
        velocity.x = -velocity.x.abs() * BOUNCE_FACTOR;
    }

    if transform.translation.y < min_y {
        transform.translation.y = min_y;
        velocity.y = velocity.y.abs() * BOUNCE_FACTOR;
    } else if transform.translation.y > max_y {
        transform.translation.y = max_y;
        velocity.y = -velocity.y.abs() * BOUNCE_FACTOR;
    }

    **velocity *= (-FRICTION * dt).exp();
    if velocity.length() < STOP_SPEED {
        **velocity = Vec2::ZERO;
    }
    //endregion
}