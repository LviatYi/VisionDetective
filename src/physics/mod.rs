use crate::PLAYER_RADIUS;
use crate::coin::player::PlayerCoin;
use crate::obstacle::Obstacle;
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{Component, Deref, DerefMut, Query, Res, Time, Transform, Without};

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
    obstacle_query: Query<(&Transform, &Obstacle), Without<PlayerCoin>>,
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

    resolve_obstacle_collisions(&mut transform, &mut velocity, &obstacle_query);

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
}

fn resolve_obstacle_collisions(
    transform: &mut Transform,
    velocity: &mut Velocity,
    obstacle_query: &Query<(&Transform, &Obstacle), Without<PlayerCoin>>,
) {
    let mut player_position = transform.translation.truncate();

    for (obstacle_transform, obstacle) in obstacle_query.iter() {
        let world_path = obstacle.world_path(obstacle_transform);
        if let Some((normal, penetration)) =
            collide_circle_with_polygon(player_position, PLAYER_RADIUS, &world_path)
        {
            player_position += normal * penetration;

            let planar_velocity = velocity.truncate();
            let normal_speed = planar_velocity.dot(normal);
            if normal_speed < 0.0 {
                let tangential_velocity = planar_velocity - normal * normal_speed;
                let reflected_normal_velocity = -normal * normal_speed * BOUNCE_FACTOR;
                let next_velocity = tangential_velocity + reflected_normal_velocity;

                velocity.x = next_velocity.x;
                velocity.y = next_velocity.y;
            }
        }
    }

    transform.translation.x = player_position.x;
    transform.translation.y = player_position.y;
}

fn collide_circle_with_polygon(
    circle_center: Vec2,
    radius: f32,
    polygon: &[Vec2],
) -> Option<(Vec2, f32)> {
    if polygon.len() < 3 {
        return None;
    }

    let signed_area = polygon_signed_area(polygon);
    let inside = point_in_polygon(circle_center, polygon);

    let mut closest_distance_squared = f32::MAX;
    let mut closest_point = Vec2::ZERO;
    let mut closest_edge = Vec2::ZERO;

    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        let point_on_edge = closest_point_on_segment(circle_center, start, end);
        let distance_squared = circle_center.distance_squared(point_on_edge);

        if distance_squared < closest_distance_squared {
            closest_distance_squared = distance_squared;
            closest_point = point_on_edge;
            closest_edge = end - start;
        }
    }

    let distance = closest_distance_squared.sqrt();
    let fallback_normal = edge_outward_normal(closest_edge, signed_area);

    if inside {
        return Some((fallback_normal, radius + distance));
    }

    if closest_distance_squared > radius * radius {
        return None;
    }

    let normal = if distance > f32::EPSILON {
        (circle_center - closest_point) / distance
    } else {
        fallback_normal
    };

    Some((normal, radius - distance))
}

fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut inside = false;
    let mut previous = polygon[polygon.len() - 1];

    for &current in polygon {
        let intersects = (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x;
        if intersects {
            inside = !inside;
        }
        previous = current;
    }

    inside
}

fn closest_point_on_segment(point: Vec2, start: Vec2, end: Vec2) -> Vec2 {
    let edge = end - start;
    let edge_length_squared = edge.length_squared();
    if edge_length_squared <= f32::EPSILON {
        return start;
    }

    let t = (point - start).dot(edge) / edge_length_squared;
    start + edge * t.clamp(0.0, 1.0)
}

fn polygon_signed_area(polygon: &[Vec2]) -> f32 {
    let mut area = 0.0;

    for index in 0..polygon.len() {
        let current = polygon[index];
        let next = polygon[(index + 1) % polygon.len()];
        area += current.x * next.y - next.x * current.y;
    }

    area * 0.5
}

fn edge_outward_normal(edge: Vec2, signed_area: f32) -> Vec2 {
    if edge.length_squared() <= f32::EPSILON {
        return Vec2::Y;
    }

    let right_hand_normal = Vec2::new(edge.y, -edge.x).normalize();
    signed_area.signum() * right_hand_normal
}
