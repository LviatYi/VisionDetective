use crate::PLAYER_RADIUS;
use crate::coin::player::PlayerCoin;
use bevy::math::Vec3;
use bevy::prelude::{Component, Deref, DerefMut, Query, Res, Time, Transform};

pub const ARENA_HALF_WIDTH: f32 = 500.0;
pub const ARENA_HALF_HEIGHT: f32 = 280.0;
pub const BOUNCE_FACTOR: f32 = 0.72;
pub const FRICTION: f32 = -1000.0;
pub const STOP_SPEED: f32 = 8.0;
pub const GRAVITY: f32 = -9.8;

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
    coin.sim_z = (coin.sim_z + velocity.z * dt).max(0.0);
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

    velocity.z += GRAVITY * dt;
    let on_ground = coin.sim_z <= 0.0;
    if velocity.z < 0.0 && on_ground {
        coin.sim_z = 0.0;
        velocity.z = 0.0;
    }
    if on_ground {
        velocity.x = velocity.x.signum() * (velocity.x.abs() + FRICTION * dt).max(0.0);
        velocity.y = velocity.y.signum() * (velocity.y.abs() + FRICTION * dt).max(0.0);
    }

    //endregion
}
