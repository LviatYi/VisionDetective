use crate::coin::player::PlayerCoin;
use crate::config::GameConfig;
use bevy::prelude::*;
use serde::Deserialize;
use std::marker::PhantomData;

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

#[derive(Component, Default)]
pub struct Interactive;

#[derive(Component, Default)]
pub struct HelloWorldInteraction;

#[derive(Resource, Default)]
struct ActiveInteraction {
    current: Option<Entity>,
    previous: Option<Entity>,
}

pub trait CardInteraction: Component {
    fn on_enter(&self, entity: Entity, card: &Card) {
        let _ = (entity, card);
    }

    fn on_exit(&self, entity: Entity, card: &Card) {
        let _ = (entity, card);
    }
}

struct InteractionPlugin<T: CardInteraction>(PhantomData<T>);

impl<T: CardInteraction> Default for InteractionPlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl CardInteraction for HelloWorldInteraction {
    fn on_enter(&self, _entity: Entity, _card: &Card) {
        info!("hello world");
    }
}

impl Plugin for CardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveInteraction>();
        app.add_systems(Update, (spawn_card_visuals, update_active_interaction));
        app.add_plugins(InteractionPlugin::<HelloWorldInteraction>::default());
    }
}

impl<T: CardInteraction> Plugin for InteractionPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, dispatch_interaction_events::<T>);
    }
}

fn spawn_card(
    commands: &mut Commands,
    transform: Transform,
    card: Card,
    appearance: CardKind,
) -> Entity {
    commands.spawn((transform, card, appearance)).id()
}

pub fn spawn_scenery_card(commands: &mut Commands, transform: Transform, title: String) -> Entity {
    spawn_card(commands, transform, Card { title }, CardKind::Scenery)
}

pub fn spawn_obstacle_card(commands: &mut Commands, transform: Transform, title: String) -> Entity {
    spawn_card(commands, transform, Card { title }, CardKind::Obstacle)
}

pub fn spawn_interaction_card(
    commands: &mut Commands,
    transform: Transform,
    title: String,
) -> Entity {
    spawn_card(commands, transform, Card { title }, CardKind::Interaction)
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

fn update_active_interaction(
    config: Res<GameConfig>,
    player_query: Query<&Transform, With<PlayerCoin>>,
    interaction_query: Query<(Entity, &Card, &GlobalTransform), With<Interactive>>,
    mut active_interaction: ResMut<ActiveInteraction>,
) {
    let Ok(player_transform) = player_query.single() else {
        active_interaction.previous = active_interaction.current.take();
        return;
    };

    let player_position = player_transform.translation.truncate();
    let player_radius = config.visuals.player_radius;

    let next = interaction_query
        .iter()
        .filter(|(_, card, transform)| {
            card.intersect_circle(transform, player_position, player_radius)
        })
        .min_by(|(entity_a, _, transform_a), (entity_b, _, transform_b)| {
            let distance_a = player_position.distance_squared(transform_a.translation().truncate());
            let distance_b = player_position.distance_squared(transform_b.translation().truncate());

            distance_a
                .partial_cmp(&distance_b)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| entity_a.index().cmp(&entity_b.index()))
        })
        .map(|(entity, _, _)| entity);

    active_interaction.previous = active_interaction.current;
    active_interaction.current = next;
}

fn dispatch_interaction_events<T: CardInteraction>(
    active_interaction: Res<ActiveInteraction>,
    interaction_query: Query<(Entity, &Card, &T)>,
) {
    if !active_interaction.is_changed() {
        return;
    }

    if let Some(entity) = active_interaction.previous
        && active_interaction.current != Some(entity)
        && let Ok((entity, card, interaction)) = interaction_query.get(entity)
    {
        interaction.on_exit(entity, card);
    }

    if let Some(entity) = active_interaction.current
        && active_interaction.previous != Some(entity)
        && let Ok((entity, card, interaction)) = interaction_query.get(entity)
    {
        interaction.on_enter(entity, card);
    }
}
