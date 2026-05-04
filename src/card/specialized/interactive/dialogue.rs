use crate::card::Card;
use crate::card::specialized::interactive::CardInteraction;
use crate::register_card_interaction;
use bevy::log::info;
use bevy::prelude::Entity;
use serde::{Deserialize, Serialize};

/// One node in a card-driven dialogue flow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueNode {
    /// character ID
    pub source: u32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueInteractionParams {
    #[serde(default)]
    pub nodes: Vec<DialogueNode>,
}

/// Interaction component for dialogue cards.
pub struct DialogueInteraction {
    pub param: DialogueInteractionParams,
}

impl From<DialogueInteractionParams> for DialogueInteraction {
    fn from(value: DialogueInteractionParams) -> Self {
        Self { param: value }
    }
}

impl CardInteraction for DialogueInteraction {
    fn on_enter(&self, entity: Entity, card: &Card) {
        let entry = self
            .param
            .nodes
            .first()
            .map(|node| format!("{}: {}", node.source, node.text));

        match entry {
            Some(entry) => {
                info!("card='{}' entity={entity:?} entry={entry}", card.title);
            }
            None => {
                info!(
                    "no entry for card {}, {}",
                    card.title,
                    card.instance_type.get_prefab_id()
                );
            }
        }
    }
}

register_card_interaction!("dialogue", DialogueInteractionParams, DialogueInteraction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialogue_interaction_params_parse_node_shape() {
        let params = serde_json::from_value::<DialogueInteractionParams>(serde_json::json!({
            "nodes": [
                {
                    "source": 1,
                    "text": "我们先从这里开始调查。",
                }
            ]
        }))
        .expect("dialogue interaction params should parse");

        assert_eq!(params.nodes.len(), 1);
        assert_eq!(params.nodes[0].source, 1);
        assert_eq!(params.nodes[0].text, "我们先从这里开始调查。");
    }
}
