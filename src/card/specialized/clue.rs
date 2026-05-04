use crate::card::card_params::CardSpecialized;
use crate::card::{CARD_SIZE, CardKind};
use crate::coin::player::PlayerCoin;
use crate::config::GameConfig;
use crate::game_view::{AppView, GameState};
use crate::physics::PlayerCoinStopped;
use crate::physics::obstacle::Obstacle;
use crate::physics::vision::compute_visible_points;
use crate::register_card_specialized_param;
use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::EntityCommands;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use serde::{Deserialize, Serialize};

pub struct ClueCardPlugin;

#[derive(Component, Debug, Clone)]
pub struct ClueCard {
    pub revealed: bool,
    pub reveal_threshold: f32,
    illuminated_regions: Vec<Vec<Vec2>>,
}

#[derive(Component)]
struct ClueQuestionMark;

#[derive(Component)]
struct ClueIllumination;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClueCardParams {
    #[serde(default = "default_reveal_threshold")]
    pub reveal_threshold: f32,
    #[serde(default = "default_sample_count")]
    pub sample_count: usize,
}

impl Default for ClueCardParams {
    fn default() -> Self {
        Self {
            reveal_threshold: default_reveal_threshold(),
            sample_count: default_sample_count(),
        }
    }
}

impl CardSpecialized for ClueCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Clue
    }

    fn insert_components(&self, entity: &mut EntityCommands<'_>) {
        entity.insert(ClueCard {
            revealed: false,
            reveal_threshold: self.reveal_threshold.clamp(0.0, 1.0),
            illuminated_regions: Vec::new(),
        });
    }
}

impl Plugin for ClueCardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            setup_clue_visuals.run_if(in_state(GameState::InGame).or(in_state(AppView::Editor))),
        )
        .add_systems(
            Update,
            reveal_clues_on_player_stop.run_if(in_state(GameState::InGame)),
        );
    }
}

fn setup_clue_visuals(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config: Res<GameConfig>,
    clue_query: Query<(Entity, &ClueCard, Option<&Children>)>,
    marker_query: Query<(), With<ClueQuestionMark>>,
) {
    for (entity, clue, children) in &clue_query {
        if clue.revealed {
            continue;
        }

        let has_marker = children
            .map(|children| children.iter().any(|child| marker_query.get(child).is_ok()))
            .unwrap_or(false);
        if has_marker {
            continue;
        }

        commands.entity(entity).with_children(|parent| {
            parent.spawn((
                Text2d::new("?"),
                TextFont {
                    font: asset_server.load(config.assets.default_font.clone()),
                    font_size: CARD_SIZE.y * 0.42,
                    ..default()
                },
                TextColor(Color::srgb(0.08, 0.08, 0.09)),
                Anchor::CENTER,
                Transform::from_xyz(0.0, 0.0, 0.42),
                ClueQuestionMark,
            ));
        });
    }
}

fn reveal_clues_on_player_stop(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut stopped_events: MessageReader<PlayerCoinStopped>,
    mut clue_query: Query<(Entity, &mut ClueCard, &GlobalTransform, Option<&Children>)>,
    obstacle_query: Query<(&Transform, &Obstacle), Without<PlayerCoin>>,
    marker_query: Query<(), With<ClueQuestionMark>>,
    illumination_query: Query<(), With<ClueIllumination>>,
    mut illumination_mesh_query: Query<&mut Mesh2d, With<ClueIllumination>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for event in stopped_events.read() {
        for (entity, mut clue, transform, children) in &mut clue_query {
            if clue.revealed {
                continue;
            }

            let Some(stamp) =
                build_visible_clue_stamp(&config, event.position, transform, &obstacle_query)
            else {
                continue;
            };
            clue.illuminated_regions.push(stamp);

            let (merged_mesh, illuminated_area) = build_merged_illumination_mesh(&clue);
            let coverage = illuminated_area / card_area();

            if coverage >= clue.reveal_threshold {
                clue.revealed = true;
                despawn_clue_visual_feedback(
                    &mut commands,
                    children,
                    &marker_query,
                    &illumination_query,
                );
                continue;
            }

            update_clue_illumination_visual(
                &mut commands,
                &config,
                entity,
                children,
                merged_mesh,
                &illumination_query,
                &mut illumination_mesh_query,
                meshes.as_mut(),
                materials.as_mut(),
            );
        }
    }
}

fn despawn_clue_visual_feedback(
    commands: &mut Commands,
    children: Option<&Children>,
    marker_query: &Query<(), With<ClueQuestionMark>>,
    illumination_query: &Query<(), With<ClueIllumination>>,
) {
    let Some(children) = children else {
        return;
    };

    for child in children {
        if marker_query.get(*child).is_ok() || illumination_query.get(*child).is_ok() {
            commands.entity(*child).despawn();
        }
    }
}

fn update_clue_illumination_visual(
    commands: &mut Commands,
    config: &GameConfig,
    entity: Entity,
    children: Option<&Children>,
    merged_mesh: Option<Mesh>,
    illumination_query: &Query<(), With<ClueIllumination>>,
    illumination_mesh_query: &mut Query<&mut Mesh2d, With<ClueIllumination>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) {
    let existing_illumination = children.and_then(|children| {
        children
            .iter()
            .find(|child| illumination_query.get(*child).is_ok())
    });

    let Some(mesh) = merged_mesh else {
        if let Some(illumination) = existing_illumination {
            commands.entity(illumination).despawn();
        }
        return;
    };

    let mesh_handle = meshes.add(mesh);
    if let Some(illumination) = existing_illumination
        && let Ok(mut mesh) = illumination_mesh_query.get_mut(illumination)
    {
        *mesh = Mesh2d(mesh_handle);
        return;
    }

    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            Mesh2d(mesh_handle),
            MeshMaterial2d(materials.add(config.cards.fill_color(CardKind::Interaction))),
            Transform::from_xyz(0.0, 0.0, 0.36),
            ClueIllumination,
        ));
    });
}

fn build_visible_clue_stamp(
    config: &GameConfig,
    origin: Vec2,
    transform: &GlobalTransform,
    obstacle_query: &Query<(&Transform, &Obstacle), Without<PlayerCoin>>,
) -> Option<Vec<Vec2>> {
    let visible_points = compute_visible_points(config, origin, obstacle_query);
    if visible_points.len() < 3 {
        return None;
    }

    let world_to_local = transform.to_matrix().inverse();
    let mut local_polygon = Vec::with_capacity(visible_points.len() + 1);
    local_polygon.push(
        world_to_local
            .transform_point3(origin.extend(0.0))
            .truncate(),
    );

    for point in visible_points {
        local_polygon.push(
            world_to_local
                .transform_point3(point.extend(0.0))
                .truncate(),
        );
    }

    let clipped = clip_polygon_to_card(local_polygon);
    if clipped.len() < 3 {
        None
    } else {
        Some(remove_near_duplicate_points(clipped))
    }
}

fn build_merged_illumination_mesh(clue: &ClueCard) -> (Option<Mesh>, f32) {
    let slabs = build_union_slabs(&clue.illuminated_regions);
    if slabs.is_empty() {
        return (None, 0.0);
    }

    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    let mut area = 0.0;

    for slab in slabs {
        if slab.width() <= GEOMETRY_EPSILON {
            continue;
        }

        let low_left = slab.low_edge.y_at(slab.left);
        let low_right = slab.low_edge.y_at(slab.right);
        let high_left = slab.high_edge.y_at(slab.left);
        let high_right = slab.high_edge.y_at(slab.right);

        if high_left <= low_left + GEOMETRY_EPSILON && high_right <= low_right + GEOMETRY_EPSILON {
            continue;
        }

        let corners = [
            Vec2::new(slab.left, low_left),
            Vec2::new(slab.right, low_right),
            Vec2::new(slab.right, high_right),
            Vec2::new(slab.left, high_left),
        ];
        let base = positions.len() as u32;

        for point in corners {
            positions.push([point.x, point.y, 0.0]);
            let uv = (point / CARD_SIZE) + Vec2::splat(0.5);
            uvs.push([uv.x, uv.y]);
        }

        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        area += slab_area(low_left, low_right, high_left, high_right, slab.width());
    }

    if positions.is_empty() {
        return (None, 0.0);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    (Some(mesh), area)
}

fn build_union_slabs(polygons: &[Vec<Vec2>]) -> Vec<UnionSlab> {
    let xs = collect_union_x_events(polygons);
    if xs.len() < 2 {
        return Vec::new();
    }

    let mut slabs = Vec::new();
    for window in xs.windows(2) {
        let left = window[0];
        let right = window[1];
        if right - left <= GEOMETRY_EPSILON {
            continue;
        }

        let mid_x = (left + right) * 0.5;
        let intervals = merged_vertical_intervals(polygons, mid_x);
        for interval in intervals {
            slabs.push(UnionSlab {
                left,
                right,
                low_edge: interval.low_edge,
                high_edge: interval.high_edge,
            });
        }
    }

    slabs
}

fn collect_union_x_events(polygons: &[Vec<Vec2>]) -> Vec<f32> {
    let mut xs = vec![-CARD_SIZE.x * 0.5, CARD_SIZE.x * 0.5];
    let edges = collect_polygon_edges(polygons);

    for polygon in polygons {
        for point in polygon {
            xs.push(point.x);
        }
    }

    for left_index in 0..edges.len() {
        for right_index in left_index + 1..edges.len() {
            if let Some(intersection) = segment_intersection(edges[left_index], edges[right_index])
            {
                xs.push(intersection.x);
            }
        }
    }

    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    xs.dedup_by(|a, b| (*a - *b).abs() <= GEOMETRY_EPSILON);
    xs.into_iter()
        .filter(|x| *x >= -CARD_SIZE.x * 0.5 - GEOMETRY_EPSILON)
        .filter(|x| *x <= CARD_SIZE.x * 0.5 + GEOMETRY_EPSILON)
        .collect()
}

fn merged_vertical_intervals(polygons: &[Vec<Vec2>], x: f32) -> Vec<VerticalInterval> {
    let mut intervals = Vec::new();
    for polygon in polygons {
        intervals.extend(vertical_intervals_for_polygon(polygon, x));
    }

    if intervals.is_empty() {
        return intervals;
    }

    intervals.sort_by(|a, b| {
        a.low
            .partial_cmp(&b.low)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut merged = Vec::new();
    let mut current = intervals[0];
    for interval in intervals.into_iter().skip(1) {
        if interval.low <= current.high + GEOMETRY_EPSILON {
            if interval.low < current.low {
                current.low = interval.low;
                current.low_edge = interval.low_edge;
            }
            if interval.high > current.high {
                current.high = interval.high;
                current.high_edge = interval.high_edge;
            }
        } else {
            merged.push(current);
            current = interval;
        }
    }
    merged.push(current);
    merged
}

fn vertical_intervals_for_polygon(polygon: &[Vec2], x: f32) -> Vec<VerticalInterval> {
    let edges = polygon_edges(polygon);
    let mut hits = Vec::new();

    for edge in edges {
        if !edge.crosses_vertical_line(x) {
            continue;
        }

        hits.push((edge.y_at(x), edge));
    }

    hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut intervals = Vec::new();
    for pair in hits.chunks_exact(2) {
        let (first_y, first_edge) = pair[0];
        let (second_y, second_edge) = pair[1];
        if second_y - first_y <= GEOMETRY_EPSILON {
            continue;
        }

        intervals.push(VerticalInterval {
            low: first_y,
            high: second_y,
            low_edge: first_edge,
            high_edge: second_edge,
        });
    }

    intervals
}

fn collect_polygon_edges(polygons: &[Vec<Vec2>]) -> Vec<PolygonEdge> {
    polygons
        .iter()
        .flat_map(|polygon| polygon_edges(polygon))
        .collect()
}

fn polygon_edges(polygon: &[Vec2]) -> Vec<PolygonEdge> {
    let mut edges = Vec::new();
    if polygon.len() < 2 {
        return edges;
    }

    for index in 0..polygon.len() {
        edges.push(PolygonEdge {
            start: polygon[index],
            end: polygon[(index + 1) % polygon.len()],
        });
    }
    edges
}

fn clip_polygon_to_card(polygon: Vec<Vec2>) -> Vec<Vec2> {
    let half = CARD_SIZE * 0.5;
    let polygon = clip_polygon_edge(polygon, ClipEdge::Left(-half.x));
    let polygon = clip_polygon_edge(polygon, ClipEdge::Right(half.x));
    let polygon = clip_polygon_edge(polygon, ClipEdge::Bottom(-half.y));
    clip_polygon_edge(polygon, ClipEdge::Top(half.y))
}

fn clip_polygon_edge(polygon: Vec<Vec2>, edge: ClipEdge) -> Vec<Vec2> {
    if polygon.is_empty() {
        return polygon;
    }

    let mut clipped = Vec::with_capacity(polygon.len() + 1);
    let mut previous = *polygon.last().expect("polygon is not empty");
    let mut previous_inside = point_inside_edge(previous, edge);

    for current in polygon {
        let current_inside = point_inside_edge(current, edge);
        if current_inside {
            if !previous_inside {
                clipped.push(edge_intersection(previous, current, edge));
            }
            clipped.push(current);
        } else if previous_inside {
            clipped.push(edge_intersection(previous, current, edge));
        }

        previous = current;
        previous_inside = current_inside;
    }

    clipped
}

fn point_inside_edge(point: Vec2, edge: ClipEdge) -> bool {
    match edge {
        ClipEdge::Left(x) => point.x >= x,
        ClipEdge::Right(x) => point.x <= x,
        ClipEdge::Bottom(y) => point.y >= y,
        ClipEdge::Top(y) => point.y <= y,
    }
}

fn edge_intersection(from: Vec2, to: Vec2, edge: ClipEdge) -> Vec2 {
    let delta = to - from;
    match edge {
        ClipEdge::Left(x) | ClipEdge::Right(x) => {
            if delta.x.abs() <= f32::EPSILON {
                return Vec2::new(x, from.y);
            }
            let t = (x - from.x) / delta.x;
            Vec2::new(x, from.y + delta.y * t)
        }
        ClipEdge::Bottom(y) | ClipEdge::Top(y) => {
            if delta.y.abs() <= f32::EPSILON {
                return Vec2::new(from.x, y);
            }
            let t = (y - from.y) / delta.y;
            Vec2::new(from.x + delta.x * t, y)
        }
    }
}

fn remove_near_duplicate_points(points: Vec<Vec2>) -> Vec<Vec2> {
    let mut result = Vec::new();
    for point in points {
        if result
            .last()
            .map(|last: &Vec2| last.distance_squared(point) <= GEOMETRY_EPSILON_SQUARED)
            .unwrap_or(false)
        {
            continue;
        }
        result.push(point);
    }

    if result.len() > 1
        && result[0].distance_squared(*result.last().expect("result is not empty"))
            <= GEOMETRY_EPSILON_SQUARED
    {
        result.pop();
    }
    result
}

fn segment_intersection(left: PolygonEdge, right: PolygonEdge) -> Option<Vec2> {
    let p = left.start;
    let r = left.end - left.start;
    let q = right.start;
    let s = right.end - right.start;
    let denominator = cross_2d(r, s);
    if denominator.abs() <= GEOMETRY_EPSILON {
        return None;
    }

    let q_minus_p = q - p;
    let t = cross_2d(q_minus_p, s) / denominator;
    let u = cross_2d(q_minus_p, r) / denominator;

    if (-GEOMETRY_EPSILON..=1.0 + GEOMETRY_EPSILON).contains(&t)
        && (-GEOMETRY_EPSILON..=1.0 + GEOMETRY_EPSILON).contains(&u)
    {
        Some(p + r * t)
    } else {
        None
    }
}

fn slab_area(low_left: f32, low_right: f32, high_left: f32, high_right: f32, width: f32) -> f32 {
    let left_height = (high_left - low_left).max(0.0);
    let right_height = (high_right - low_right).max(0.0);
    (left_height + right_height) * 0.5 * width
}

fn card_area() -> f32 {
    CARD_SIZE.x * CARD_SIZE.y
}

fn cross_2d(left: Vec2, right: Vec2) -> f32 {
    left.x * right.y - left.y * right.x
}

#[derive(Clone, Copy)]
struct UnionSlab {
    left: f32,
    right: f32,
    low_edge: PolygonEdge,
    high_edge: PolygonEdge,
}

impl UnionSlab {
    fn width(&self) -> f32 {
        self.right - self.left
    }
}

#[derive(Clone, Copy)]
struct VerticalInterval {
    low: f32,
    high: f32,
    low_edge: PolygonEdge,
    high_edge: PolygonEdge,
}

#[derive(Clone, Copy)]
struct PolygonEdge {
    start: Vec2,
    end: Vec2,
}

impl PolygonEdge {
    fn crosses_vertical_line(&self, x: f32) -> bool {
        let min_x = self.start.x.min(self.end.x);
        let max_x = self.start.x.max(self.end.x);
        min_x < x && x < max_x
    }

    fn y_at(&self, x: f32) -> f32 {
        let dx = self.end.x - self.start.x;
        if dx.abs() <= f32::EPSILON {
            return self.start.y;
        }

        let t = (x - self.start.x) / dx;
        self.start.y + (self.end.y - self.start.y) * t
    }
}

#[derive(Clone, Copy)]
enum ClipEdge {
    Left(f32),
    Right(f32),
    Bottom(f32),
    Top(f32),
}

fn default_reveal_threshold() -> f32 {
    0.55
}

fn default_sample_count() -> usize {
    5
}

const GEOMETRY_EPSILON: f32 = 0.001;
const GEOMETRY_EPSILON_SQUARED: f32 = GEOMETRY_EPSILON * GEOMETRY_EPSILON;

register_card_specialized_param!("clue", ClueCardParams);
