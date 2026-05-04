use crate::GameView;
use crate::card::specialized::interactive::CardInteractionEntered;
use crate::config::GameConfig;
use crate::register_card_interaction;
use bevy::app::{App, Update};
use bevy::ecs::system::EntityCommands;
use bevy::input::ButtonInput;
use bevy::picking::pointer::PointerButton;
use bevy::picking::prelude::{Click, Pointer};
use bevy::prelude::{
    BackgroundColor, Color, Commands, Component, Entity, IntoScheduleConfigs, KeyCode,
    MessageReader, Node, On, Pickable, PositionType, Query, Res, ResMut, Resource, Text, TextColor,
    TextFont, UiRect, With, Without, default, in_state, percent, px,
};
use serde::{Deserialize, Serialize};

const DIALOGUE_PANEL_HEIGHT: f32 = 168.0;
const DIALOGUE_PANEL_PADDING_X: f32 = 34.0;
const DIALOGUE_PANEL_PADDING_Y: f32 = 22.0;
const DIALOGUE_NAME_FONT_SIZE: f32 = 20.0;
const DIALOGUE_TEXT_FONT_SIZE: f32 = 26.0;

/// One node in a card-driven dialogue flow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueNode {
    pub id: u32,
    /// Character ID.
    pub source: u32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueInteractionParams {
    #[serde(default)]
    pub nodes: Vec<DialogueNode>,
}

/// Interaction component for dialogue cards.
#[derive(Component)]
pub struct DialogueInteraction {
    pub param: DialogueInteractionParams,
}

#[derive(Resource, Default)]
struct DialogueState {
    active: Option<ActiveDialogue>,
}

impl DialogueState {
    fn current_node(&self) -> Option<&DialogueNode> {
        let active = self.active.as_ref()?;
        active.nodes.get(active.current_index)
    }

    fn next_id(&self) -> Option<usize> {
        let active = self.active.as_ref()?;
        (active.current_index + 1 < active.nodes.len()).then_some(active.current_index + 1)
    }

    fn push_dialogue(&mut self) {
        match self.next_id() {
            None => {
                self.active = None;
            }
            Some(next_id) => match self.active.as_mut() {
                None => {
                    self.active = None;
                }
                Some(active) => {
                    active.current_index = next_id;
                }
            },
        }
    }
}

struct ActiveDialogue {
    nodes: Vec<DialogueNode>,
    current_index: usize,
}

#[derive(Component)]
struct DialogueUiRoot;

#[derive(Component)]
struct DialogueSpeakerText;

#[derive(Component)]
struct DialogueBodyText;

impl From<DialogueInteractionParams> for DialogueInteraction {
    fn from(value: DialogueInteractionParams) -> Self {
        Self { param: value }
    }
}

fn insert_dialogue_interaction(params: DialogueInteractionParams, entity: &mut EntityCommands<'_>) {
    entity.insert(DialogueInteraction::from(params));
}

pub(super) fn register_dialogue_systems(app: &mut App) {
    app.init_resource::<DialogueState>();
    app.add_observer(start_dialogues_from_interaction);
    app.add_systems(
        Update,
        (advance_dialogue_input, sync_dialogue_ui)
            .chain()
            .run_if(in_state(crate::game_view::GameState::InGame)),
    );
}

fn start_dialogues_from_interaction(
    event: On<CardInteractionEntered>,
    interaction_query: Query<&DialogueInteraction>,
    mut state: ResMut<DialogueState>,
) {
    let Ok(interaction) = interaction_query.get(event.entity) else {
        return;
    };

    if interaction.param.nodes.is_empty() {
        bevy::log::warn!("dialogue card {:?} has no dialogue nodes", event.entity);
        state.active = None;
        return;
    }

    state.active = Some(ActiveDialogue {
        nodes: interaction.param.nodes.clone(),
        current_index: 0,
    });
}

fn advance_dialogue_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut click_events: MessageReader<Pointer<Click>>,
    mut state: ResMut<DialogueState>,
) {
    let clicked = click_events
        .read()
        .any(|event| event.button == PointerButton::Primary);
    if keyboard_input.just_pressed(KeyCode::Space) || clicked {
        state.push_dialogue();
    }
}

fn sync_dialogue_ui(
    mut commands: Commands,
    asset_server: Res<bevy::prelude::AssetServer>,
    config: Res<GameConfig>,
    state: Res<DialogueState>,
    root_query: Query<Entity, With<DialogueUiRoot>>,
    mut speaker_query: Query<&mut Text, (With<DialogueSpeakerText>, Without<DialogueUiRoot>)>,
    mut body_query: Query<&mut Text, (With<DialogueBodyText>, Without<DialogueSpeakerText>)>,
) {
    let root = root_query.iter().next();

    if state.active.is_none() {
        if let Some(root) = root {
            commands.entity(root).despawn();
        }
        return;
    }

    let Some(node) = state.current_node() else {
        return;
    };

    if root.is_none() {
        spawn_dialogue_ui(&mut commands, asset_server.as_ref(), &config, node);
    }

    for mut text in &mut speaker_query {
        **text = format!("角色 {}", node.source);
    }
    for mut text in &mut body_query {
        **text = node.text.clone();
    }
}

fn spawn_dialogue_ui(
    commands: &mut Commands,
    asset_server: &bevy::prelude::AssetServer,
    config: &GameConfig,
    node: &DialogueNode,
) {
    let font = asset_server.load(config.assets.default_font.clone());

    commands
        .spawn((
            Node {
                width: percent(100.0),
                height: percent(100.0),
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
            Pickable::default(),
            DialogueUiRoot,
            GameView,
        ))
        .with_children(|modal| {
            modal
                .spawn((
                    Node {
                        width: percent(100.0),
                        height: px(DIALOGUE_PANEL_HEIGHT),
                        position_type: PositionType::Absolute,
                        left: px(0.0),
                        bottom: px(0.0),
                        padding: UiRect::axes(
                            px(DIALOGUE_PANEL_PADDING_X),
                            px(DIALOGUE_PANEL_PADDING_Y),
                        ),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.5)),
                    Pickable::default(),
                ))
                .with_children(|panel| {
                    panel
                        .spawn((
                            Node {
                                width: percent(100.0),
                                height: percent(100.0),
                                flex_direction: bevy::prelude::FlexDirection::Column,
                                row_gap: px(12.0),
                                ..default()
                            },
                            Pickable::IGNORE,
                        ))
                        .with_children(|content| {
                            content.spawn((
                                Text::new(format!("角色 {}", node.source)),
                                TextFont {
                                    font: font.clone(),
                                    font_size: DIALOGUE_NAME_FONT_SIZE,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.38, 0.28, 0.02)),
                                DialogueSpeakerText,
                                Pickable::IGNORE,
                            ));
                            content.spawn((
                                Text::new(node.text.clone()),
                                TextFont {
                                    font,
                                    font_size: DIALOGUE_TEXT_FONT_SIZE,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.08, 0.09, 0.11)),
                                DialogueBodyText,
                                Pickable::IGNORE,
                            ));
                        });
                });
        });
}

register_card_interaction!(
    "dialogue",
    DialogueInteractionParams,
    inserter = insert_dialogue_interaction,
    systems = register_dialogue_systems
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialogue_interaction_params_parse_node_shape() {
        let params = serde_json::from_value::<DialogueInteractionParams>(serde_json::json!({
            "nodes": [
                {
                    "id": 0,
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
