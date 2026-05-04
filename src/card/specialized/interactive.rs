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
    Component, DetectChanges, Entity, GlobalTransform, IntoScheduleConfigs, Message, MessageWriter,
    Query, Res, ResMut, Resource, Transform, With, in_state,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Marker component for cards that participate in interaction handling.
#[derive(Component, Default)]
pub struct Interactive;

#[derive(Message, Clone, Copy, Debug)]
pub struct CardInteractionEntered {
    pub entity: Entity,
    pub prefab_id: u32,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct CardInteractionExited {
    pub entity: Entity,
    pub prefab_id: u32,
}

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

impl CardSpecialized for InteractionCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Interaction
    }

    fn insert_components(&self, entity: &mut EntityCommands<'_>) {
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

        entity.insert(Interactive);

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

/// Plugin that wires the interaction-card runtime systems.
pub struct InteractionCardPlugin;

impl Plugin for InteractionCardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveInteraction>();
        app.add_message::<CardInteractionEntered>();
        app.add_message::<CardInteractionExited>();
        app.add_systems(
            Update,
            (
                update_active_interaction,
                dispatch_interaction_events.after(update_active_interaction),
                (
                    hello_world::log_hello_world_interactions,
                    dialogue::log_dialogue_interactions,
                )
                    .after(dispatch_interaction_events),
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// Function signature used to insert one interaction component from raw JSON.
pub type CardInteractionComponentInserter = fn(&Value, &mut EntityCommands<'_>) -> Result<()>;

/// Static registration entry collected through `inventory`.
pub(super) struct CardInteractionRegistration {
    pub type_id: &'static str,
    pub json_src_inserter: CardInteractionComponentInserter,
}

impl CardInteractionRegistration {
    pub const fn new(
        type_id: &'static str,
        json_src_inserter: CardInteractionComponentInserter,
    ) -> Self {
        Self {
            type_id,
            json_src_inserter,
        }
    }

    fn insert(&self, json: &Value, entity: &mut EntityCommands<'_>) -> Result<()> {
        (self.json_src_inserter)(json, entity)
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
            let order_a = transform_a.translation().z;
            let order_b = transform_b.translation().z;

            order_a
                .partial_cmp(&order_b)
                .unwrap_or(std::cmp::Ordering::Equal)
                .reverse()
                .then_with(|| entity_a.index().cmp(&entity_b.index()))
        })
        .map(|(entity, _, _)| entity);

    active_interaction.previous = active_interaction.current;
    active_interaction.current = next;
}

fn dispatch_interaction_events(
    active_interaction: Res<ActiveInteraction>,
    interaction_query: Query<(Entity, &Card), With<Interactive>>,
    mut entered_writer: MessageWriter<CardInteractionEntered>,
    mut exited_writer: MessageWriter<CardInteractionExited>,
) {
    if !active_interaction.is_changed() {
        return;
    }

    if let Some(entity) = active_interaction.previous
        && active_interaction.current != Some(entity)
        && let Ok((entity, card)) = interaction_query.get(entity)
    {
        exited_writer.write(CardInteractionExited {
            entity,
            prefab_id: card.instance_type.get_prefab_id(),
        });
    }

    if let Some(entity) = active_interaction.current
        && active_interaction.previous != Some(entity)
        && let Ok((entity, card)) = interaction_query.get(entity)
    {
        entered_writer.write(CardInteractionEntered {
            entity,
            prefab_id: card.instance_type.get_prefab_id(),
        });
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
