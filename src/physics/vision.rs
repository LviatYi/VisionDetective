use crate::coin::player::PlayerCoin;
use crate::physics::obstacle::Obstacle;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

const VISION_RADIUS: f32 = 240.0;

/// The default number of rays used in the view calculation.
/// This determines the accuracy of the view circle calculation and rendering
/// (the lower the value, the fewer lines are used to represent the circle).
const VISION_RAY_COUNT: usize = 128;
const VISION_VERTEX_EPSILON: f32 = 0.0025;
const INTERSECTION_EPSILON: f32 = 0.0001;
const VISION_FILL_COLOR: Color = Color::srgba(0.92, 0.95, 0.80, 0.18);
const VISION_OUTLINE_COLOR: Color = Color::srgba(0.95, 0.98, 0.86, 0.42);

#[derive(Component)]
pub struct VisionFieldMesh;

pub fn setup_vision_system(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) {
    commands.spawn((
        Mesh2d(meshes.add(build_vision_mesh(Vec2::ZERO, &[]))),
        MeshMaterial2d(materials.add(VISION_FILL_COLOR)),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.5)),
        VisionFieldMesh,
    ));
}

pub fn update_vision_field_mesh(
    player_query: Query<&Transform, With<PlayerCoin>>,
    obstacle_query: Query<(&Transform, &Obstacle), Without<PlayerCoin>>,
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
    let visible_points = compute_visible_points(player_position, &obstacle_query);
    *mesh = build_vision_mesh(player_position, &visible_points);
}

pub fn draw_vision_radius(mut gizmos: Gizmos, player_query: Query<&Transform, With<PlayerCoin>>) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    gizmos.circle_2d(
        player_transform.translation.truncate(),
        VISION_RADIUS,
        VISION_OUTLINE_COLOR,
    );
}

fn compute_visible_points(
    origin: Vec2,
    obstacle_query: &Query<(&Transform, &Obstacle), Without<PlayerCoin>>,
) -> Vec<Vec2> {
    let mut rays_radius = Vec::with_capacity(VISION_RAY_COUNT * 2);

    for index in 0..VISION_RAY_COUNT {
        let angle = std::f32::consts::TAU * index as f32 / VISION_RAY_COUNT as f32;
        rays_radius.push(normalize_angle(angle));
    }

    for (transform, obstacle) in obstacle_query.iter() {
        let world_path = obstacle.world_path(transform);
        for &point in &world_path {
            let angle = normalize_angle((point.y - origin.y).atan2(point.x - origin.x));
            rays_radius.push(normalize_angle(angle - VISION_VERTEX_EPSILON));
            rays_radius.push(normalize_angle(angle + VISION_VERTEX_EPSILON));
        }

        for index in 0..world_path.len() {
            let start = world_path[index];
            let end = world_path[(index + 1) % world_path.len()];
            for point in segment_circle_intersections(start, end, origin, VISION_RADIUS) {
                let angle = normalize_angle((point.y - origin.y).atan2(point.x - origin.x));
                rays_radius.push(angle);
            }
        }
    }

    rays_radius.sort_by(|a, b| a.total_cmp(b));
    rays_radius.dedup_by(|a, b| (*a - *b).abs() < INTERSECTION_EPSILON);

    rays_radius
        .into_iter()
        .map(|angle| cast_visibility_ray(origin, angle, obstacle_query))
        .collect()
}

fn cast_visibility_ray(
    origin: Vec2,
    angle: f32,
    obstacle_query: &Query<(&Transform, &Obstacle), Without<PlayerCoin>>,
) -> Vec2 {
    let direction = Vec2::from_angle(angle);
    let mut closest_distance = VISION_RADIUS;

    for (transform, obstacle) in obstacle_query.iter() {
        let world_path = obstacle.world_path(transform);
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

fn ray_segment_intersection(origin: Vec2, direction: Vec2, start: Vec2, end: Vec2) -> Option<f32> {
    let segment = end - start;
    let denominator = cross(direction, segment);
    if denominator.abs() <= f32::EPSILON {
        return None;
    }

    let offset = start - origin;
    let ray_distance = cross(offset, segment) / denominator;
    let segment_distance = cross(offset, direction) / denominator;

    if ray_distance < 0.0 || ray_distance > VISION_RADIUS || !(0.0..1.0).contains(&segment_distance)
    {
        return None;
    }

    Some(ray_distance)
}

fn build_vision_mesh(center: Vec2, visible_points: &[Vec2]) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    let mut positions = vec![[center.x, center.y, 0.0]];
    let mut uvs = vec![[0.5, 0.5]];
    let mut indices = Vec::new();

    for point in visible_points {
        positions.push([point.x, point.y, 0.0]);
        let uv = ((point - center) / (VISION_RADIUS * 2.0)) + Vec2::splat(0.5);
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

fn segment_circle_intersections(start: Vec2, end: Vec2, center: Vec2, radius: f32) -> Vec<Vec2> {
    let segment = end - start;
    let from_center = start - center;
    let a = segment.length_squared();
    if a <= f32::EPSILON {
        return Vec::new();
    }

    let b = 2.0 * from_center.dot(segment);
    let c = from_center.length_squared() - radius * radius;
    let discriminant = b * b - 4.0 * a * c;

    if discriminant < -INTERSECTION_EPSILON {
        return Vec::new();
    }

    if discriminant.abs() <= INTERSECTION_EPSILON {
        let t = -b / (2.0 * a);
        if (-INTERSECTION_EPSILON..=1.0 + INTERSECTION_EPSILON).contains(&t) {
            return vec![start + segment * t.clamp(0.0, 1.0)];
        }
        return Vec::new();
    }

    let sqrt_discriminant = discriminant.sqrt();
    let t1 = (-b - sqrt_discriminant) / (2.0 * a);
    let t2 = (-b + sqrt_discriminant) / (2.0 * a);
    let mut points = Vec::with_capacity(2);

    if (-INTERSECTION_EPSILON..=1.0 + INTERSECTION_EPSILON).contains(&t1) {
        points.push(start + segment * t1.clamp(0.0, 1.0));
    }
    if (-INTERSECTION_EPSILON..=1.0 + INTERSECTION_EPSILON).contains(&t2)
        && (t2 - t1).abs() > INTERSECTION_EPSILON
    {
        points.push(start + segment * t2.clamp(0.0, 1.0));
    }

    points
}
