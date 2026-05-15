use crate::card::specialized::obstacle::{Obstacle, draw_obstacle_paths};
use crate::card::specialized::trap::draw_trap_paths;
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::{PlayerCoinBehaviorStatus, PlayerCoinState};
use crate::config::GameConfig;
use crate::scene::terrain::{draw_editor_terrain_paths, spawn_terrain_boundary_meshes};
use crate::tools::Disable;
use crate::{AppStatus, GameplaySet};
use bevy::math::{Vec2, Vec3};
use bevy::prelude::*;

pub mod area;
pub mod vision;

pub struct PhysicsPlugin;

#[derive(Component, Deref, DerefMut, Default)]
pub struct Velocity(Vec3);

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            move_player_coin_transform.in_set(GameplaySet::PlayerPhysics),
        )
        .add_systems(
            Update,
            (
                draw_obstacle_paths,
                draw_trap_paths,
                draw_editor_terrain_paths,
            )
                .run_if(in_state(AppStatus::Editor)),
        )
        .add_systems(
            Update,
            spawn_terrain_boundary_meshes.run_if(in_state(AppStatus::Game)),
        );
    }
}

pub fn move_player_coin_transform(
    config: Res<GameConfig>,
    time: Res<Time>,
    mut transform_query: Query<
        (Mut<PlayerCoinState>, &mut Transform, &mut Velocity),
        With<PlayerCoin>,
    >,
    obstacle_query: Query<(&Transform, &Obstacle), (Without<PlayerCoin>, Without<Disable>)>,
) {
    for (mut player_state, mut transform, mut velocity) in transform_query.iter_mut() {
        if player_state.is_stop() {
            continue;
        }

        let dt = time.delta_secs();

        transform.translation += velocity.with_z(0.0) * dt;
        resolve_obstacle_collisions(&config, &mut transform, &mut velocity, &obstacle_query);

        match player_state.state() {
            PlayerCoinBehaviorStatus::Upspring { .. } => {
                let sim_z = player_state.sim_z();
                player_state.set_sim_z(sim_z + velocity.z * dt);
                velocity.z += config.physics.gravity * dt;

                if player_state.sim_z() <= 0.0 {
                    player_state.set_sim_z(0.0);
                    velocity.x *= config.physics.landing_speed_loss;
                    velocity.y *= config.physics.landing_speed_loss;

                    if player_state.contact_count() >= config.physics.max_ground_bounce_count {
                        player_state.set_state(PlayerCoinBehaviorStatus::Slide);
                        velocity.z = 0.0;
                    } else {
                        velocity.z = -velocity.z * config.physics.landing_speed_loss;
                        let new_contact_count = player_state.contact_count() + 1;
                        player_state.set_state(PlayerCoinBehaviorStatus::Contact {
                            save_velocity: velocity.clone(),
                            contact_count: new_contact_count,
                        });
                    }
                }
            }
            PlayerCoinBehaviorStatus::Contact {
                save_velocity,
                contact_count,
            } => {
                velocity.0 = save_velocity;
                player_state.set_state(PlayerCoinBehaviorStatus::Upspring {
                    sim_z: 0.0,
                    contact_count,
                });
                let new_sim_z = player_state.sim_z() + velocity.z * dt;
                player_state.set_sim_z(new_sim_z);
            }
            PlayerCoinBehaviorStatus::Slide => {
                let planar_velocity = velocity.truncate();
                let planar_speed = planar_velocity.length();

                if planar_speed < config.physics.stop_speed {
                    velocity.x = 0.0;
                    velocity.y = 0.0;
                    player_state.set_state(PlayerCoinBehaviorStatus::Idle);
                } else {
                    let friction_delta = config.physics.sliding_friction * dt;
                    let next_speed = (planar_speed - friction_delta).max(0.0);
                    let next_planar_velocity = planar_velocity.normalize() * next_speed;

                    velocity.x = next_planar_velocity.x;
                    velocity.y = next_planar_velocity.y;
                }
            }
            _ => {}
        }
    }
}

fn resolve_obstacle_collisions(
    config: &GameConfig,
    transform: &mut Transform,
    velocity: &mut Velocity,
    obstacle_query: &Query<(&Transform, &Obstacle), (Without<PlayerCoin>, Without<Disable>)>,
) {
    let mut player_position = transform.translation.truncate();

    for (obstacle_transform, obstacle) in obstacle_query.iter() {
        let world_path = obstacle.world_path(obstacle_transform);
        if let Some((normal, penetration)) =
            collide_circle_with_polygon(player_position, config.visuals.player_radius, &world_path)
        {
            player_position += normal * penetration;

            let planar_velocity = velocity.truncate();
            let normal_speed = planar_velocity.dot(normal);
            if normal_speed < 0.0 {
                let tangential_velocity = planar_velocity - normal * normal_speed;
                let reflected_normal_velocity =
                    -normal * normal_speed * config.physics.bounce_factor;
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
