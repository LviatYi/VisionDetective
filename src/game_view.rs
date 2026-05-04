use crate::game_view::main_view::{
    MainMenuView, cleanup_view, handle_main_menu_buttons, setup_main_menu,
};
use bevy::app::{App, Plugin, Update};
use bevy::prelude::*;

pub struct GameViewPlugin;

impl Plugin for GameViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppScreen::MainMenu), setup_main_menu)
            .add_systems(OnExit(AppScreen::MainMenu), cleanup_view::<MainMenuView>)
            .add_systems(
                Update,
                handle_main_menu_buttons.run_if(in_state(AppScreen::MainMenu)),
            );
    }
}

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppScreen {
    #[default]
    MainMenu,
    Game,
    Editor,
}

#[derive(SubStates, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[source(AppScreen = AppScreen::Game)]
pub enum GameState {
    #[default]
    Loading,
    InGame,
}

pub mod main_view {
    use crate::asset::font;
    use crate::config::GameConfig;
    use crate::game_view::AppScreen;
    use bevy::asset::{AssetServer, Handle};
    use bevy::camera::Camera2d;
    use bevy::color::Color;
    use bevy::input::ButtonInput;
    use bevy::picking::pointer::PointerButton;
    use bevy::picking::prelude::{Pointer, Press};
    use bevy::prelude::{
        AlignItems, BackgroundColor, Button, ChildSpawnerCommands, Commands, Component, Entity,
        FlexDirection, Font, JustifyContent, KeyCode, MessageReader, NextState, Node, Pickable,
        Query, Res, ResMut, Text, TextColor, TextFont, UiRect, With, default, percent, px,
    };

    #[derive(Component)]
    pub struct MainMenuView;

    #[derive(Component)]
    pub(super) struct MainMenuButton {
        target: AppScreen,
    }

    pub(super) fn setup_main_menu(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        config: Res<GameConfig>,
    ) {
        let ui_font = font::load_assets(asset_server, &config, font::FontType::Default);

        commands.spawn((Camera2d, MainMenuView));

        commands
            .spawn((
                Node {
                    width: percent(100.0),
                    height: percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.96)),
                MainMenuView,
            ))
            .with_children(|parent| {
                parent
                    .spawn((
                        Node {
                            width: px(520.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: px(24.0),
                            padding: UiRect::all(px(32.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.10, 0.12, 0.16, 0.92)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            Text::new("Vision Detective"),
                            TextFont {
                                font: ui_font.clone(),
                                font_size: 42.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));

                        panel.spawn((
                            Text::new("选择进入游戏场景或编辑器场景"),
                            TextFont {
                                font: ui_font.clone(),
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.78, 0.82, 0.88)),
                        ));

                        spawn_main_menu_button(panel, &ui_font, "进入游戏", AppScreen::Game);
                        spawn_main_menu_button(panel, &ui_font, "进入编辑器", AppScreen::Editor);
                        panel.spawn((
                            Text::new("按 Esc 可从游戏或编辑器返回主页面"),
                            TextFont {
                                font: ui_font,
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.58, 0.66, 0.74)),
                        ));
                    });
            });
    }

    pub(super) fn spawn_main_menu_button(
        parent: &mut ChildSpawnerCommands,
        font: &Handle<Font>,
        label: &str,
        target: AppScreen,
    ) {
        parent
            .spawn((
                Button,
                Node {
                    width: px(260.0),
                    height: px(56.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.24, 0.35, 0.47)),
                Pickable::default(),
                MainMenuButton { target },
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new(label),
                    TextFont {
                        font: font.clone(),
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Pickable::IGNORE,
                ));
            });
    }

    pub(super) fn handle_main_menu_buttons(
        mut press_events: MessageReader<Pointer<Press>>,
        button_query: Query<&MainMenuButton>,
        mut next_screen: ResMut<NextState<AppScreen>>,
    ) {
        for event in press_events.read() {
            if event.button != PointerButton::Primary {
                continue;
            }
            let Ok(button) = button_query.get(event.entity) else {
                continue;
            };
            next_screen.set(button.target);
        }
    }

    pub fn handle_esc_to_main_menu(
        keyboard_input: Res<ButtonInput<KeyCode>>,
        mut editor_state: Option<ResMut<crate::editor::EditorInteractionState>>,
        mut next_screen: ResMut<NextState<AppScreen>>,
    ) {
        if editor_state
            .as_ref()
            .map(|state| state.captures_pointer())
            .unwrap_or(false)
        {
            return;
        }
        if editor_state
            .as_mut()
            .map(|state| state.take_escape_consumed())
            .unwrap_or(false)
        {
            return;
        }

        if keyboard_input.just_pressed(KeyCode::Escape) {
            next_screen.set(AppScreen::MainMenu);
        }
    }

    pub fn cleanup_view<T: Component>(mut commands: Commands, query: Query<Entity, With<T>>) {
        for entity in &query {
            commands.entity(entity).despawn();
        }
    }
}
