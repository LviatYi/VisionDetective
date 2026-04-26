use crate::PLAYER_RADIUS;
use crate::coin::player::PlayerCoin;
use bevy::math::Vec3;
use bevy::prelude::{Component, Deref, DerefMut, Query, Res, Time, Transform};

pub const ARENA_HALF_WIDTH: f32 = 500.0;
pub const ARENA_HALF_HEIGHT: f32 = 280.0;
pub const BOUNCE_FACTOR: f32 = 0.72;
pub const LANDING_SPEED_LOSS: f32 = 0.72;
pub const SLIDING_FRICTION: f32 = 180.0;
pub const STOP_SPEED: f32 = 8.0;
pub const GRAVITY: f32 = -9.8;
pub const MAX_GROUND_BOUNCE_COUNT: u8 = 2;

#[derive(Component, Deref, DerefMut, Default)]
pub struct Velocity(Vec3);

pub fn move_player_coin_transform(
    time: Res<Time>,
    mut transform_query: Query<(&mut Transform, &mut PlayerCoin, &mut Velocity)>,
) {
    let Ok((mut transform, mut coin, mut velocity)) = transform_query.single_mut() else {
        return;
    };

    let dt = time.delta_secs();
    let airborne = coin.ground_contact_count < MAX_GROUND_BOUNCE_COUNT;

    if airborne {
        coin.sim_z += velocity.z * dt;
        velocity.z += GRAVITY * dt;
    }

    transform.translation += velocity.with_z(0.0) * dt;

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

    if airborne && coin.sim_z <= 0.0 {
        coin.sim_z = 0.0;
        coin.ground_contact_count += 1;
        velocity.x *= LANDING_SPEED_LOSS;
        velocity.y *= LANDING_SPEED_LOSS;

        if coin.ground_contact_count >= MAX_GROUND_BOUNCE_COUNT {
            velocity.z = 0.0;
        } else {
            velocity.z = -velocity.z * LANDING_SPEED_LOSS;
        }
    }

    if coin.ground_contact_count >= MAX_GROUND_BOUNCE_COUNT {
        let planar_velocity = velocity.truncate();
        let planar_speed = planar_velocity.length();

        if planar_speed < STOP_SPEED {
            velocity.x = 0.0;
            velocity.y = 0.0;
        } else {
            let friction_delta = SLIDING_FRICTION * dt;
            let next_speed = (planar_speed - friction_delta).max(0.0);
            let next_planar_velocity = planar_velocity.normalize() * next_speed;

            velocity.x = next_planar_velocity.x;
            velocity.y = next_planar_velocity.y;
        }
    }

    //endregion
}
