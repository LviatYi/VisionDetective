use crate::card::card_params::{CardSpecializedParam, SpawnCardSystemParams};
use crate::card::{CardKind, CardSpecializedInstaller};
use crate::register_card_specialized_installer;
use bevy::ecs::system::EntityCommands;
use serde::{Deserialize, Serialize};

//region Installer

pub struct SceneryCardSpecializedInstaller;

impl CardSpecializedInstaller for SceneryCardSpecializedInstaller {
    type Param = SceneryCardParams;

    const TYPE_ID: &'static str = "scenery";
}

register_card_specialized_installer!(SceneryCardSpecializedInstaller);

//endregion

//region Card Specialized Param

/// Specialized parameters for scenery cards.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneryCardParams {}

impl CardSpecializedParam for SceneryCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Scenery
    }

    fn spawn_with(
        &self,
        _entity: &mut EntityCommands<'_>,
        _spawn_params: &mut SpawnCardSystemParams<'_>,
    ) {
    }
}

//endregion
