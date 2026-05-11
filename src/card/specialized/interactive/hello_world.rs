use crate::card::specialized::interactive::CardInteractionEntered;
use crate::register_card_interaction;
use bevy::ecs::system::EntityCommands;
use bevy::log::info;
use bevy::prelude::{Component, On};
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

fn insert_hello_world_interaction(
    entity: &mut EntityCommands<'_>,
    params: HelloWorldInteractionParams,
) {
    entity
        .insert(HelloWorldInteraction::from(params))
        .observe(log_hello_world_interaction);
}

fn log_hello_world_interaction(_event: On<CardInteractionEntered>) {
    info!("hello world");
}

register_card_interaction!(
    "log_hello_world",
    HelloWorldInteractionParams,
    inserter = insert_hello_world_interaction
);
