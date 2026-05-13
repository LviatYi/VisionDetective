use crate::card::Card;
use crate::card::specialized::obstacle::Obstacle;
use crate::coin::player::PlayerCoin;
use crate::config::GameConfig;
use crate::scene::SceneLayer;
use crate::tools::Disable;
use crate::{GameStatus, GameView, GameplaySet};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

pub struct VisionPlugin;

#[derive(Component)]
pub struct VisionFieldMesh;

impl Plugin for VisionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameStatus::Loading), setup_vision_system)
            .add_systems(
                Update,
                (update_vision_field_mesh, draw_vision_radius).in_set(GameplaySet::Visual),
            );
    }
}

pub fn setup_vision_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        Mesh2d(meshes.add(build_vision_mesh(&config, Vec2::ZERO, &[]))),
        MeshMaterial2d(materials.add(config.vision.fill_color())),
        Transform::from_translation(Vec3::new(
            0.0,
            0.0,
            SceneLayer::PlayerVision.get_layer_base_z(),
        )),
        VisionFieldMesh,
        GameView,
    ));
}

pub fn update_vision_field_mesh(
    config: Res<GameConfig>,
    player_query: Query<&Transform, With<PlayerCoin>>,
    obstacle_query: Query<(&Transform, &Obstacle), (Without<PlayerCoin>, Without<Disable>)>,
    vision_query: Query<&Mesh2d, With<VisionFieldMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Ok(player_transform), Ok(vision_mesh)) = (player_query.single(), vision_query.single())
    else {
        return;
    };

    let Some(mesh) = meshes.get_mut(&vision_mesh.0) else {
        return;
    };

    let player_position = player_transform.translation.truncate();
    let max_obstacle_distance = config.vision.radius + Card::SIZE.length();
    let max_obstacle_distance_squared = max_obstacle_distance * max_obstacle_distance;
    let obstacle_paths = collect_nearby_obstacle_paths(
        player_position,
        max_obstacle_distance_squared,
        &obstacle_query,
    );
    let visible_points =
        compute_visible_points_from_world_paths(&config, player_position, &obstacle_paths);
    *mesh = build_vision_mesh(&config, player_position, &visible_points);
}

pub fn draw_vision_radius(
    config: Res<GameConfig>,
    mut gizmos: Gizmos,
    player_query: Query<&Transform, With<PlayerCoin>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    draw_vision_radius_at(
        &config,
        &mut gizmos,
        player_transform.translation.truncate(),
    );
}

pub fn draw_vision_radius_at(config: &GameConfig, gizmos: &mut Gizmos, center: Vec2) {
    gizmos.circle_2d(center, config.vision.radius, config.vision.outline_color());
}

pub fn compute_visible_points<F: bevy::ecs::query::QueryFilter>(
    config: &GameConfig,
    origin: Vec2,
    obstacle_query: &Query<(&Transform, &Obstacle), F>,
) -> Vec<Vec2> {
    let obstacle_paths = obstacle_query
        .iter()
        .map(|(transform, obstacle)| obstacle.world_path(transform))
        .collect::<Vec<_>>();
    compute_visible_points_from_world_paths(config, origin, &obstacle_paths)
}

fn collect_nearby_obstacle_paths<F: bevy::ecs::query::QueryFilter>(
    origin: Vec2,
    max_distance_squared: f32,
    obstacle_query: &Query<(&Transform, &Obstacle), F>,
) -> Vec<Vec<Vec2>> {
    obstacle_query
        .iter()
        .filter_map(|(transform, obstacle)| {
            let obstacle_position = transform.translation.truncate();
            (obstacle_position.distance_squared(origin) <= max_distance_squared)
                .then(|| obstacle.world_path(transform))
        })
        .collect()
}

fn compute_visible_points_from_world_paths(
    config: &GameConfig,
    origin: Vec2,
    obstacle_paths: &[Vec<Vec2>],
) -> Vec<Vec2> {
    let mut rays_radius = Vec::with_capacity(config.vision.ray_count * 2);

    for index in 0..config.vision.ray_count {
        let angle = std::f32::consts::TAU * index as f32 / config.vision.ray_count as f32;
        rays_radius.push(normalize_angle(angle));
    }

    for world_path in obstacle_paths {
        for &point in world_path {
            let angle = normalize_angle((point.y - origin.y).atan2(point.x - origin.x));
            rays_radius.push(normalize_angle(angle - config.vision.vertex_epsilon));
            rays_radius.push(normalize_angle(angle + config.vision.vertex_epsilon));
        }

        for index in 0..world_path.len() {
            let start = world_path[index];
            let end = world_path[(index + 1) % world_path.len()];
            for point in segment_circle_intersections(&config, start, end, origin) {
                let angle = normalize_angle((point.y - origin.y).atan2(point.x - origin.x));
                rays_radius.push(angle);
            }
        }
    }

    rays_radius.sort_by(|a, b| a.total_cmp(b));
    rays_radius.dedup_by(|a, b| (*a - *b).abs() < config.vision.intersection_epsilon);

    rays_radius
        .into_iter()
        .map(|angle| cast_visibility_ray(&config, origin, angle, obstacle_paths))
        .collect()
}

fn cast_visibility_ray(
    config: &GameConfig,
    origin: Vec2,
    angle: f32,
    obstacle_paths: &[Vec<Vec2>],
) -> Vec2 {
    let direction = Vec2::from_angle(angle);
    let mut closest_distance = config.vision.radius;

    for world_path in obstacle_paths {
        if world_path.len() < 2 {
            continue;
        }

        for index in 0..world_path.len() {
            let start = world_path[index];
            let end = world_path[(index + 1) % world_path.len()];
            if let Some(distance) = ray_segment_intersection(origin, direction, start, end) {
                closest_distance = closest_distance.min(distance);
            }
        }
    }

    origin + direction * closest_distance
}

pub fn ray_segment_intersection(
    origin: Vec2,
    direction: Vec2,
    start: Vec2,
    end: Vec2,
) -> Option<f32> {
    let segment = end - start;
    let denominator = cross(direction, segment);
    if denominator.abs() <= f32::EPSILON {
        return None;
    }

    let offset = start - origin;
    let ray_distance = cross(offset, segment) / denominator;
    let segment_distance = cross(offset, direction) / denominator;

    if ray_distance < 0.0 || !(0.0..1.0).contains(&segment_distance) {
        return None;
    }

    Some(ray_distance)
}

pub fn build_vision_mesh(config: &GameConfig, center: Vec2, visible_points: &[Vec2]) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    let mut positions = vec![[center.x, center.y, 0.0]];
    let mut uvs = vec![[0.5, 0.5]];
    let mut indices = Vec::new();

    for point in visible_points {
        positions.push([point.x, point.y, 0.0]);
        let uv = ((point - center) / (config.vision.radius * 2.0)) + Vec2::splat(0.5);
        uvs.push([uv.x, uv.y]);
    }

    if visible_points.len() >= 3 {
        for index in 1..visible_points.len() {
            indices.extend_from_slice(&[0_u32, index as u32, index as u32 + 1]);
        }
        indices.extend_from_slice(&[0, visible_points.len() as u32, 1]);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn normalize_angle(angle: f32) -> f32 {
    angle.rem_euclid(std::f32::consts::TAU)
}

fn segment_circle_intersections(
    config: &GameConfig,
    start: Vec2,
    end: Vec2,
    center: Vec2,
) -> Vec<Vec2> {
    let segment = end - start;
    let from_center = start - center;
    let a = segment.length_squared();
    if a <= f32::EPSILON {
        return Vec::new();
    }

    let b = 2.0 * from_center.dot(segment);
    let c = from_center.length_squared() - config.vision.radius * config.vision.radius;
    let discriminant = b * b - 4.0 * a * c;

    if discriminant < -config.vision.intersection_epsilon {
        return Vec::new();
    }

    if discriminant.abs() <= config.vision.intersection_epsilon {
        let t = -b / (2.0 * a);
        if (-config.vision.intersection_epsilon..=1.0 + config.vision.intersection_epsilon)
            .contains(&t)
        {
            return vec![start + segment * t.clamp(0.0, 1.0)];
        }
        return Vec::new();
    }

    let sqrt_discriminant = discriminant.sqrt();
    let t1 = (-b - sqrt_discriminant) / (2.0 * a);
    let t2 = (-b + sqrt_discriminant) / (2.0 * a);
    let mut points = Vec::with_capacity(2);

    if (-config.vision.intersection_epsilon..=1.0 + config.vision.intersection_epsilon)
        .contains(&t1)
    {
        points.push(start + segment * t1.clamp(0.0, 1.0));
    }
    if (-config.vision.intersection_epsilon..=1.0 + config.vision.intersection_epsilon)
        .contains(&t2)
        && (t2 - t1).abs() > config.vision.intersection_epsilon
    {
        points.push(start + segment * t2.clamp(0.0, 1.0));
    }

    points
}
