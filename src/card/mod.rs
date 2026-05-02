pub mod card_params;
pub mod specialized;

use crate::card::card_params::{CardParam, CardSpecializedRegistry};
use crate::config::card_config::CardPresetsConfig;
use crate::config::GameConfig;
use bevy::prelude::*;
use serde::Deserialize;

pub struct CardPlugin;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    Scenery,
    Obstacle,
    Interaction,
}

pub const STANDARD_CARD_SIZE: Vec2 = Vec2::new(53.9, 85.6);
pub const IN_GAME_CARD_SIZE_SCALE: f32 = 2.0;
pub const CARD_SIZE: Vec2 = Vec2::new(
    STANDARD_CARD_SIZE.x * IN_GAME_CARD_SIZE_SCALE,
    STANDARD_CARD_SIZE.y * IN_GAME_CARD_SIZE_SCALE,
);

#[derive(Component, Clone, Debug)]
pub struct Card {
    pub title: String,
    // pub image: Image,
}

impl Card {
    pub fn intersect_circle(&self, transform: &GlobalTransform, point: Vec2, radius: f32) -> bool {
        let local_point = transform
            .to_matrix()
            .inverse()
            .transform_point3(point.extend(0.0))
            .truncate();

        let half_size = CARD_SIZE * 0.5;

        let closest_x = local_point.x.clamp(-half_size.x, half_size.x);
        let closest_y = local_point.y.clamp(-half_size.y, half_size.y);

        let distance_x = local_point.x - closest_x;
        let distance_y = local_point.y - closest_y;

        (distance_x * distance_x + distance_y * distance_y) <= (radius * radius)
    }

    pub fn contains_point(&self, transform: &GlobalTransform, point: Vec2) -> bool {
        self.intersect_circle(transform, point, 0.0)
    }
}

fn spawn_card_visuals(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    card_query: Query<(Entity, &CardKind), Added<Card>>,
) {
    for (entity, kind) in &card_query {
        commands.entity(entity).insert((
            Mesh2d(meshes.add(Rectangle::new(CARD_SIZE.x, CARD_SIZE.y))),
            MeshMaterial2d(materials.add(config.cards.fill_color(*kind))),
        ));
    }
}

impl Plugin for CardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CardSpecializedRegistry>();
        app.add_systems(Update, spawn_card_visuals);
        app.add_plugins(specialized::interactive::InteractionCardPlugin);
    }
}

pub fn spawn_card_by_card_param(
    commands: &mut Commands,
    card_param: &CardParam,
    card_presets_config: &CardPresetsConfig,
    card_specialized_registry: &CardSpecializedRegistry,
) -> Entity {
    let appearance = card_param.load_appearance(card_presets_config);
    let specialized =
        card_param.load_specialized_config(card_presets_config, card_specialized_registry);

    let mut entity = commands.spawn((
        Transform::from_translation(card_param.scene_param.position.extend(0.0))
            .with_rotation(Quat::from_rotation_z(card_param.scene_param.rotation)),
        Card {
            title: appearance.title.clone(),
        },
    ));

    if let Some(specialized) = specialized {
        entity.insert(specialized.kind());
        specialized.insert_components(&mut entity);
    } else {
        entity.insert(CardKind::Scenery);
    }

    entity.id()
}
