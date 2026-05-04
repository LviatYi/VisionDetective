use crate::card::specialized::interactive::CardInteractionEntered;
use crate::register_card_interaction;
use bevy::log::info;
use bevy::prelude::{Component, MessageReader, Query};
use serde::{Deserialize, Serialize};

/// Parameters for the hello-world interaction action.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HelloWorldInteractionParams {}

/// Example interaction effect used by current demo cards.
#[derive(Component, Default)]
pub struct HelloWorldInteraction;

impl From<HelloWorldInteractionParams> for HelloWorldInteraction {
    fn from(_value: HelloWorldInteractionParams) -> Self {
        Self
    }
}

pub(super) fn log_hello_world_interactions(
    mut entered_events: MessageReader<CardInteractionEntered>,
    interaction_query: Query<&HelloWorldInteraction>,
) {
    for event in entered_events.read() {
        if interaction_query.get(event.entity).is_ok() {
            info!("hello world");
        }
    }
}

register_card_interaction!(
    "log_hello_world",
    HelloWorldInteractionParams,
    HelloWorldInteraction
);
