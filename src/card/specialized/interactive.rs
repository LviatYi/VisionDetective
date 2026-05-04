mod dialogue;
mod hello_world;

use crate::card::card_params::CardSpecialized;
use crate::card::{Card, CardKind};
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::PlayerCoinState;
use crate::config::GameConfig;
use crate::game_view::GameState;
use crate::register_card_specialized_param;
use anyhow::Result;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::EntityCommands;
use bevy::prelude::{
    Component, DetectChanges, Entity, GlobalTransform, IntoScheduleConfigs, Query, Res, ResMut,
    Resource, Transform, With, in_state,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::marker::PhantomData;

/// Marker component for cards that participate in interaction handling.
#[derive(Component, Default)]
pub struct Interactive;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardInteractionConfig {
    /// Registered interaction action type name.
    #[serde(rename = "type")]
    pub type_id: String,

    /// Raw JSON payload for the interaction action.
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionCardParams {
    pub interaction: CardInteractionConfig,
}

pub trait CardInteraction: Component {
    fn on_enter(&self, entity: Entity, card: &Card) {
        let _ = (entity, card);
    }

    fn on_exit(&self, entity: Entity, card: &Card) {
        let _ = (entity, card);
    }
}

impl CardSpecialized for InteractionCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Interaction
    }

    fn insert_components(&self, entity: &mut EntityCommands<'_>) {
        entity.insert(Interactive);

        let Some(registration) = inventory::iter::<CardInteractionRegistration>
            .into_iter()
            .find(|registration| registration.type_id == self.interaction.type_id)
        else {
            bevy::log::warn!(
                "unknown card interaction type: {}",
                self.interaction.type_id
            );
            return;
        };

        if let Err(error) = registration.insert(&self.interaction.params, entity) {
            bevy::log::warn!(
                "failed to deserialize card interaction {}: {error}",
                self.interaction.type_id
            );
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
        app.add_systems(
            Update,
            update_active_interaction.run_if(in_state(GameState::InGame)),
        );

        for registration in inventory::iter::<CardInteractionRegistration> {
            registration.register_plugin(app);
        }
    }
}

impl<T: CardInteraction> Plugin for InteractionPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            dispatch_interaction_events::<T>.run_if(in_state(GameState::InGame)),
        );
    }
}

pub fn register_interaction_plugin<T: CardInteraction>(app: &mut App) {
    app.add_plugins(InteractionPlugin::<T>::default());
}

/// Function signature used to insert one interaction component from raw JSON.
pub type CardInteractionComponentInserter = fn(&Value, &mut EntityCommands<'_>) -> Result<()>;

/// Function signature used to register systems for one interaction component type.
pub type CardInteractionPluginInjector = fn(&mut App);

/// Static registration entry collected through `inventory`.
pub(super) struct CardInteractionRegistration {
    pub type_id: &'static str,
    pub json_src_inserter: CardInteractionComponentInserter,
    pub plugin_registrar: CardInteractionPluginInjector,
}

impl CardInteractionRegistration {
    pub const fn new(
        type_id: &'static str,
        json_src_inserter: CardInteractionComponentInserter,
        plugin_registrar: CardInteractionPluginInjector,
    ) -> Self {
        Self {
            type_id,
            json_src_inserter,
            plugin_registrar,
        }
    }

    fn insert(&self, json: &Value, entity: &mut EntityCommands<'_>) -> Result<()> {
        (self.json_src_inserter)(json, entity)
    }

    fn register_plugin(&self, app: &mut App) {
        (self.plugin_registrar)(app)
    }
}

inventory::collect!(CardInteractionRegistration);

#[macro_export]
macro_rules! register_card_interaction {
    ($name:expr, $param_type:ty, $component_type:ty) => {
        inventory::submit! {
            $crate::card::specialized::interactive::CardInteractionRegistration::new(
                $name,
                |value: &serde_json::Value, entity: &mut bevy::ecs::system::EntityCommands<'_>| -> anyhow::Result<()> {
                    let params = serde_json::from_value::<$param_type>(value.clone())?;
                    entity.insert(<$component_type as From<$param_type>>::from(params));
                    Ok(())
                },
                |app: &mut bevy::app::App| {
                    $crate::card::specialized::interactive::register_interaction_plugin::<$component_type>(app);
                }
            )
        }
    };
}

fn update_active_interaction(
    config: Res<GameConfig>,
    player_state: Res<PlayerCoinState>,
    player_query: Query<&Transform, With<PlayerCoin>>,
    interaction_query: Query<(Entity, &Card, &GlobalTransform), With<Interactive>>,
    mut active_interaction: ResMut<ActiveInteraction>,
) {
    if !player_state.is_idle() {
        active_interaction.previous = active_interaction.current.take();
        return;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_can_find_hello_world_interaction_action() {
        let registration = inventory::iter::<CardInteractionRegistration>
            .into_iter()
            .find(|registration| registration.type_id == "log_hello_world");

        assert!(registration.is_some());
    }

    #[test]
    fn registry_can_find_dialogue_interaction_action() {
        let registration = inventory::iter::<CardInteractionRegistration>
            .into_iter()
            .find(|registration| registration.type_id == "dialogue");

        assert!(registration.is_some());
    }

    #[test]
    fn interaction_card_params_parse_registered_action_shape() {
        let params = serde_json::from_value::<InteractionCardParams>(serde_json::json!({
            "interaction": {
                "type": "log_hello_world",
                "params": {}
            }
        }))
        .expect("interaction card params should parse");

        assert_eq!(params.interaction.type_id, "log_hello_world");
        assert_eq!(params.interaction.params, serde_json::json!({}));
    }
}
