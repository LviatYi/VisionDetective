use crate::card::card_params::CardSpecializedRegistry;
use crate::card::{Card, CardPlugin, spawn_card_visuals};
use crate::coin::player::PlayerCoin;
use crate::config::GameConfig;
use bevy::app::{App, Plugin, Update};
use bevy::log::info;
use bevy::prelude::{
    Component, DetectChanges, Entity, GlobalTransform, Query, Res, ResMut, Resource, Transform,
    With,
};
use std::marker::PhantomData;

#[derive(Component, Default)]
pub struct Interactive;

pub trait CardInteraction: Component {
    fn on_enter(&self, entity: Entity, card: &Card) {
        let _ = (entity, card);
    }

    fn on_exit(&self, entity: Entity, card: &Card) {
        let _ = (entity, card);
    }
}

#[derive(Component, Default)]
pub struct HelloWorldInteraction;

impl CardInteraction for HelloWorldInteraction {
    fn on_enter(&self, _entity: Entity, _card: &Card) {
        info!("hello world");
    }
}

#[derive(Resource, Default)]
struct ActiveInteraction {
    current: Option<Entity>,
    previous: Option<Entity>,
}

struct InteractionPlugin<T: CardInteraction>(PhantomData<T>);

impl<T: CardInteraction> Default for InteractionPlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl Plugin for CardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveInteraction>();
        app.init_resource::<CardSpecializedRegistry>();
        app.add_systems(Update, (spawn_card_visuals, update_active_interaction));
        app.add_plugins(InteractionPlugin::<HelloWorldInteraction>::default());
    }
}

impl<T: CardInteraction> Plugin for InteractionPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, dispatch_interaction_events::<T>);
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
