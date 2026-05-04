use crate::card::Card;
use crate::card::specialized::interactive::CardInteraction;
use crate::register_card_interaction;
use bevy::log::info;
use bevy::prelude::Entity;
use serde::{Deserialize, Serialize};

/// Parameters for the hello-world interaction action.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HelloWorldInteractionParams {}

/// Example interaction effect used by current demo cards.
#[derive(Default)]
pub struct HelloWorldInteraction;

impl From<HelloWorldInteractionParams> for HelloWorldInteraction {
    fn from(_value: HelloWorldInteractionParams) -> Self {
        Self
    }
}

impl CardInteraction for HelloWorldInteraction {
    fn on_enter(&self, _entity: Entity, _card: &Card) {
        info!("hello world");
    }
}

register_card_interaction!(
    "log_hello_world",
    HelloWorldInteractionParams,
    HelloWorldInteraction
);
