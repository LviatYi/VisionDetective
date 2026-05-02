use crate::card::card_params::CardSpecialized;
use crate::card::CardKind;
use crate::register_card_specialized_param;
use bevy::ecs::system::EntityCommands;
use serde::{Deserialize, Serialize};

/// Specialized parameters for scenery cards.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneryCardParams {}

impl CardSpecialized for SceneryCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Scenery
    }

    fn insert_components(&self, _entity: &mut EntityCommands<'_>) {}
}

register_card_specialized_param!("scenery", SceneryCardParams);
