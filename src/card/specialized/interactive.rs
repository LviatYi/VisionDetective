use crate::card::card_params::CardSpecialized;
use crate::card::{Card, CardKind};
use crate::coin::player::PlayerCoin;
use crate::config::GameConfig;
use crate::register_card_specialized_param;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::EntityCommands;
use bevy::log::info;
use bevy::prelude::{
    Component, DetectChanges, Entity, GlobalTransform, Query, Res, ResMut, Resource, Transform,
    With,
};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// Marker component for cards that participate in interaction handling.
#[derive(Component, Default)]
pub struct Interactive;

/// Interaction effect variants supported by interactive cards.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardInteractionType {
    LogHelloWorld,
}

/// Specialized parameters for interaction cards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionCardParams {
    pub interaction: CardInteractionType,
}

/// Runtime behavior contract for interaction effect components.
pub trait CardInteraction: Component {
    fn on_enter(&self, entity: Entity, card: &Card) {
        let _ = (entity, card);
    }

    fn on_exit(&self, entity: Entity, card: &Card) {
        let _ = (entity, card);
    }
}

/// Example interaction effect used by current demo cards.
#[derive(Component, Default)]
pub struct HelloWorldInteraction;

impl CardInteraction for HelloWorldInteraction {
    fn on_enter(&self, _entity: Entity, _card: &Card) {
        info!("hello world");
    }
}

impl CardSpecialized for InteractionCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Interaction
    }

    fn insert_components(&self, entity: &mut EntityCommands<'_>) {
        entity.insert(Interactive);

        match self.interaction {
            CardInteractionType::LogHelloWorld => {
                entity.insert(HelloWorldInteraction);
            }
        }
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

/// Plugin that wires the interaction-card runtime systems.
pub struct InteractionCardPlugin;

impl Plugin for InteractionCardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveInteraction>();
        app.add_systems(Update, update_active_interaction);
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

register_card_specialized_param!("interactive", InteractionCardParams);
