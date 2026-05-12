mod dialogue;
mod hello_world;

use crate::card::card_params::{
    CardRuntimeSpecializedConfig, CardSpawnParams, CardSpecializedConfigData, CardSpecializedParam,
};
use crate::card::{Card, CardKind, CardSpecializedInstaller};
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::PlayerCoinState;
use crate::config::GameConfig;
use crate::editor::EditorRuntimeSpecializedParam;
use crate::game_view::{AppView, GameState};
use crate::progress::GameProgress;
use crate::tools::Disable;
use crate::{register_card_editor_systems, register_card_specialized_installer};
use anyhow::Result;
use bevy::app::{App, Update};
use bevy::ecs::system::EntityCommands;
use bevy::prelude::{
    Commands, Component, DetectChanges, Entity, EntityEvent, GlobalTransform, IntoScheduleConfigs,
    On, Query, Res, ResMut, Resource, Transform, With, Without, in_state,
};
use serde::{Deserialize, Serialize};

pub struct InteractiveCardSpecializedInstaller;

impl CardSpecializedInstaller for InteractiveCardSpecializedInstaller {
    type Param = InteractionCardParams;

    const TYPE_ID: &'static str = "interactive";

    fn install(app: &mut App) {
        app.init_resource::<ActiveInteraction>();
        for registration in inventory::iter::<CardInteractionRegistration> {
            if let Some(install_systems) = registration.system_installer {
                install_systems(app);
            }
        }
        app.add_systems(
            Update,
            (
                update_active_interaction,
                clear_disabled_active_interaction,
                dispatch_interaction_events,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
    }
}

register_card_specialized_installer!(InteractiveCardSpecializedInstaller);

/// Marker component for cards that participate in interaction handling.
#[derive(Component, Default)]
pub struct Interactive {
    /// release the state_key when the user interacts
    state_key: Option<String>,
}

//region Enter & Exit Event

#[derive(EntityEvent, Component, Clone, Copy, Debug)]
pub struct CardInteractionEntered {
    pub entity: Entity,
    pub prefab_id: u32,
}

#[derive(EntityEvent, Component, Clone, Copy, Debug)]
pub struct CardInteractionExited {
    pub entity: Entity,
    pub prefab_id: u32,
}

//endregion

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionCardParams {
    /// Registered interaction action type name.
    #[serde(rename = "type")]
    pub type_id: String,

    /// Raw JSON payload for the interaction action.
    #[serde(default)]
    pub params: serde_json::Value,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_key: Option<String>,
}

impl CardSpecializedParam for InteractionCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Interaction
    }

    fn spawn_with(&self, entity: &mut EntityCommands<'_>, _spawn_params: &mut CardSpawnParams<'_>) {
        let Some(registration) = inventory::iter::<CardInteractionRegistration>
            .into_iter()
            .find(|registration| registration.type_id == self.type_id)
        else {
            bevy::log::warn!("unknown card interaction type: {}", self.type_id);
            return;
        };

        entity
            .insert(Interactive {
                state_key: self.state_key.clone(),
            })
            .observe(unlock_progress_from_interaction);

        if let Err(error) = registration.insert(entity, &self.params) {
            bevy::log::warn!(
                "failed to deserialize card interaction {}: {error}",
                self.type_id
            );
        }
    }
}

//region Progress State

fn unlock_progress_from_interaction(
    event: On<CardInteractionEntered>,
    query: Query<&Interactive>,
    mut progress: ResMut<GameProgress>,
) {
    let Ok(interactive) = query.get(event.entity) else {
        return;
    };

    let Some(state_key) = interactive.state_key.as_ref() else {
        return;
    };

    bevy::log::info!("unlock key {}", state_key);
    progress.unlock(state_key);
}
//endregion

#[derive(Resource, Default)]
struct ActiveInteraction {
    current: Option<Entity>,
    previous: Option<Entity>,
}

fn update_active_interaction(
    config: Res<GameConfig>,
    player_state: Res<PlayerCoinState>,
    player_query: Query<&Transform, With<PlayerCoin>>,
    interaction_query: Query<
        (Entity, &Card, &GlobalTransform),
        (With<Interactive>, Without<Disable>),
    >,
    mut active_interaction: ResMut<ActiveInteraction>,
) {
    if !player_state.is_changed() && !player_state.is_stop() {
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

fn clear_disabled_active_interaction(
    disable_query: Query<(), With<Disable>>,
    mut active_interaction: ResMut<ActiveInteraction>,
) {
    let Some(current) = active_interaction.current else {
        return;
    };

    if !disable_query.contains(current) {
        return;
    }

    active_interaction.previous = active_interaction.current.take();
}

fn dispatch_interaction_events(
    active_interaction: Res<ActiveInteraction>,
    interaction_query: Query<(Entity, &Card), With<Interactive>>,
    mut commands: Commands,
) {
    if !active_interaction.is_changed() {
        return;
    }

    if let Some(entity) = active_interaction.previous
        && active_interaction.current != Some(entity)
        && let Ok((entity, card)) = interaction_query.get(entity)
    {
        commands.trigger(CardInteractionExited {
            entity,
            prefab_id: card.instance_type.get_prefab_id(),
        });
    }

    if let Some(entity) = active_interaction.current
        && active_interaction.previous != Some(entity)
        && let Ok((entity, card)) = interaction_query.get(entity)
    {
        commands.trigger(CardInteractionEntered {
            entity,
            prefab_id: card.instance_type.get_prefab_id(),
        });
    }
}

//region Card Interaction Registration

/// Function signature used to insert one interaction component with raw JSON param.
pub type CardInteractionComponentInserter =
    fn(&mut EntityCommands<'_>, &serde_json::Value) -> Result<()>;
pub type CardInteractionSystemInstaller = fn(&mut App);

/// Static registration entry collected through `inventory`.
pub(super) struct CardInteractionRegistration {
    pub type_id: &'static str,
    pub json_src_inserter: Option<CardInteractionComponentInserter>,
    pub system_installer: Option<CardInteractionSystemInstaller>,
}

impl CardInteractionRegistration {
    pub const fn new(
        type_id: &'static str,
        json_src_inserter: Option<CardInteractionComponentInserter>,
        system_installer: Option<CardInteractionSystemInstaller>,
    ) -> Self {
        Self {
            type_id,
            json_src_inserter,
            system_installer,
        }
    }

    fn insert(&self, entity: &mut EntityCommands<'_>, json: &serde_json::Value) -> Result<()> {
        self.json_src_inserter
            .map(|func| func(entity, json))
            .unwrap_or(Ok(()))
    }
}

inventory::collect!(CardInteractionRegistration);

#[macro_export]
macro_rules! register_card_interaction {
    (
        $name:expr,
        $param_type:ty
        $(, inserter = $inserter:path)?
        $(, systems = $system_installer:path)?
        $(,)?
    ) => {
        inventory::submit! {
            $crate::card::specialized::interactive::CardInteractionRegistration::new(
                $name,
                $crate::register_card_interaction!(@__option_inserter $param_type $(, $inserter)?),
                $crate::register_card_interaction!(@__option $($system_installer)?),
            )
        }
    };

    (@__option_inserter $param_type:ty, $inserter:path) => {
        Some(|entity: &mut bevy::ecs::system::EntityCommands<'_>, value: &serde_json::Value| -> anyhow::Result<()> {
            let params = serde_json::from_value::<$param_type>(value.clone())?;
            $inserter(entity, params);
            Ok(())
        })
    };

    (@__option_inserter $param_type:ty) => {
        None
    };

    (@__option $system_installer:path) => {
        Some($system_installer)
    };

    (@__option) => {
        None
    };
}
//endregion

//region Editor

fn register_editor_systems(app: &mut App) {
    app.add_systems(
        Update,
        (update_editor_runtime_params,).run_if(in_state(AppView::Editor)),
    );
}

fn update_editor_runtime_params(mut commands: Commands, query: Query<(Entity, &Interactive)>) {
    for (entity, interactive) in &query {
        let Some(state_key) = interactive.state_key.as_ref() else {
            continue;
        };

        if let Ok(mut entity) = commands.get_entity(entity) {
            entity.try_insert(EditorRuntimeSpecializedParam(
                CardRuntimeSpecializedConfig {
                    data: CardSpecializedConfigData {
                        type_id: "interactive".to_string(),
                        params: serde_json::json!({
                            "state_key":state_key,
                        }),
                    },
                },
            ));
        }
    }
}

register_card_editor_systems!("interactive", register_editor_systems);
//endregion

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
            "type": "log_hello_world",
            "params": {}
        }))
        .expect("interaction card params should parse");

        assert_eq!(params.type_id, "log_hello_world");
        assert_eq!(params.params, serde_json::json!({}));
    }
}
