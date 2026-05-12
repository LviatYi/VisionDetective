use crate::card::Card;
use crate::coin::player::PlayerCoin;
use crate::config::GameConfig;
use crate::tools::Disable;
use bevy::picking::backend::prelude::*;
use bevy::prelude::*;
use crate::AppStatus;

pub struct VisionPickingPlugin;

impl Plugin for VisionPickingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            update_scene_pointer_hits.in_set(PickingSystems::Backend),
        );
    }
}

fn update_scene_pointer_hits(
    config: Res<GameConfig>,
    app_view: Res<State<AppStatus>>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    camera_query: Query<(Entity, &Camera, &GlobalTransform, &Projection), With<Camera2d>>,
    card_query: Query<(
        Entity,
        &Card,
        &GlobalTransform,
        Option<&InheritedVisibility>,
        Option<&Disable>,
    )>,
    player_query: Query<(Entity, &GlobalTransform, Option<&InheritedVisibility>), With<PlayerCoin>>,
    mut pointer_hits_writer: MessageWriter<PointerHits>,
) {
    let Some((camera_entity, camera, camera_transform, projection)) = camera_query
        .iter()
        .filter(|(_, camera, _, _)| camera.is_active)
        .max_by_key(|(_, camera, _, _)| camera.order)
    else {
        return;
    };

    for (pointer_id, pointer_location) in &pointers {
        let Some(location) = pointer_location.location() else {
            continue;
        };
        let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, location.position)
        else {
            continue;
        };

        let mut picks = Vec::new();
        for (entity, card, transform, visibility, disable) in &card_query {
            if app_view.get() == &AppStatus::Game && disable.is_some() {
                continue;
            }
            if visibility.is_some_and(|visibility| !visibility.get()) {
                continue;
            }
            if card.contains_point(transform, world_position) {
                let position = world_position.extend(transform.translation().z);
                let depth = picking_depth(camera_transform, projection, position);
                picks.push((
                    entity,
                    HitData::new(camera_entity, depth, Some(position), None),
                ));
            }
        }

        for (entity, transform, visibility) in &player_query {
            if visibility.is_some_and(|visibility| !visibility.get()) {
                continue;
            }
            let position = transform.translation();
            if position.truncate().distance(world_position) <= config.visuals.player_radius {
                let hit_position = world_position.extend(position.z);
                let depth = picking_depth(camera_transform, projection, hit_position);
                picks.push((
                    entity,
                    HitData::new(camera_entity, depth, Some(hit_position), None),
                ));
            }
        }

        pointer_hits_writer.write(PointerHits::new(*pointer_id, picks, camera.order as f32));
    }
}

fn picking_depth(
    camera_transform: &GlobalTransform,
    projection: &Projection,
    world_position: Vec3,
) -> f32 {
    let camera_position = camera_transform
        .affine()
        .inverse()
        .transform_point3(world_position);

    match projection {
        Projection::Orthographic(orthographic) => -orthographic.near - camera_position.z,
        _ => -camera_position.z,
    }
}
