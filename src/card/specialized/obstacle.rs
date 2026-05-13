use crate::card::card_params::{CardSpawnParams, CardSpecializedParam};
use crate::card::{CardKind, CardSpecializedInstaller};
use crate::physics::area::ShapeType;
use crate::physics::obstacle::Obstacle;
use crate::register_card_specialized_installer;
use bevy::ecs::system::EntityCommands;
use serde::{Deserialize, Serialize};

pub struct ObstacleCardSpecializedInstaller;

impl CardSpecializedInstaller for ObstacleCardSpecializedInstaller {
    type Param = ObstacleCardParams;

    const TYPE_ID: &'static str = "obstacle";
}

register_card_specialized_installer!(ObstacleCardSpecializedInstaller);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObstacleCardParams {
    pub shape_def: ShapeType,
}

impl CardSpecializedParam for ObstacleCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Obstacle
    }

    fn spawn_with(&self, entity: &mut EntityCommands<'_>, spawn_params: &mut CardSpawnParams<'_>) {
        entity.insert(Obstacle::new(
            self.shape_def.sample_path(&spawn_params.config.cards),
        ));
    }
}
