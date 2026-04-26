use crate::coin::player::PlayerCoin;
use crate::config::GameConfig;
use bevy::prelude::*;

pub struct CardPlugin;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    Scenery,
    Obstacle,
    Interaction,
}

#[derive(Component, Clone, Debug)]
pub struct Card {
    pub kind: CardKind,
    pub size: Vec2,
    pub title: String,
}

impl Card {
    pub fn contains_point(&self, transform: &GlobalTransform, point: Vec2) -> bool {
        let local_point = transform
            .to_matrix()
            .inverse()
            .transform_point3(point.extend(0.0))
            .truncate();
        let half_size = self.size * 0.5;

        local_point.x >= -half_size.x
            && local_point.x <= half_size.x
            && local_point.y >= -half_size.y
            && local_point.y <= half_size.y
    }
}

#[derive(Component, Default)]
pub struct HelloWorldInteraction;

#[derive(Component, Default)]
pub struct InteractionState {
    player_inside: bool,
}

impl Plugin for CardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_card_visuals, handle_hello_world_interactions),
        );
    }
}

fn spawn_card_visuals(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    card_query: Query<(Entity, &Card), Added<Card>>,
) {
    for (entity, card) in &card_query {
        commands.entity(entity).insert((
            Mesh2d(meshes.add(Rectangle::new(card.size.x, card.size.y))),
            MeshMaterial2d(materials.add(config.cards.fill_color(card.kind))),
        ));
    }
}

fn handle_hello_world_interactions(
    player_query: Query<&Transform, With<PlayerCoin>>,
    mut interaction_query: Query<
        (&Card, &GlobalTransform, &mut InteractionState),
        With<HelloWorldInteraction>,
    >,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let player_position = player_transform.translation.truncate();

    for (card, transform, mut state) in &mut interaction_query {
        let is_inside = card.contains_point(transform, player_position);

        if is_inside && !state.player_inside {
            info!("hello world");
        }

        state.player_inside = is_inside;
    }
}
