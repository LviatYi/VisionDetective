use crate::card::CardKind;
use crate::card::card_params::{CardSpawnParams, CardSpecializedParam};
use crate::physics::area::ShapeType;
use bevy::prelude::EntityCommands;
use serde::{Deserialize, Serialize};

pub struct TrapCardSpecializedInstaller;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapCardParams {
    pub shape_def: ShapeType,
}

impl CardSpecializedParam for TrapCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Trap
    }

    fn spawn_with(&self, entity: &mut EntityCommands<'_>, spawn_params: &mut CardSpawnParams<'_>) {
        // entity.insert();
    }
}
