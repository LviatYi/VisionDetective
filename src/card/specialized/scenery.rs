use crate::card::CardKind;
use crate::card::card_params::{CardSpawnParams, CardSpecialized};
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

    fn spawn_with(
        &self,
        _entity: &mut EntityCommands<'_>,
        _spawn_params: &mut CardSpawnParams<'_>,
    ) {
    }
}

register_card_specialized_param!("scenery", SceneryCardParams);
