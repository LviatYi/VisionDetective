pub mod asset;
pub mod card;
pub mod coin;
pub mod config;
pub mod editor;
mod game_view;
pub mod physics;
pub mod scene;

use crate::asset::font;
use crate::card::CardPlugin;
use crate::coin::player::controller::EjectInputState;
use crate::coin::player::PlayerPlugin;
use crate::config::GameConfig;
use crate::editor::EditorPlugin;
use crate::game_view::main_view::{cleanup_view, handle_esc_to_main_menu};
use crate::game_view::{AppScreen, GameViewPlugin};
use crate::physics::vision::VisionPlugin;
use crate::physics::PhysicsPlugin;
use crate::physics::Velocity;
use crate::scene::demo_level::spawn_demo_cards;
use bevy::prelude::*;
use bevy::window::WindowResolution;

#[derive(Component)]
pub struct GameView;

#[derive(Component)]
struct StatusText;

fn main() {
    let config = GameConfig::load();

    App::new()
        .insert_resource(ClearColor(config.window.clear_color()))
        .insert_resource(config.clone())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: config.window.title.clone().into(),
                resolution: WindowResolution::new(config.window.width, config.window.height),
                resizable: config.window.resizable,
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppScreen>()
        .add_plugins((
            GameViewPlugin,
            PlayerPlugin,
            PhysicsPlugin,
            VisionPlugin,
            CardPlugin,
            EditorPlugin,
        ))
        .add_systems(OnEnter(AppScreen::Game), setup_game_scene)
        .add_systems(Update, update_status_text.run_if(in_state(AppScreen::Game)))
        .add_systems(
            Update,
            handle_esc_to_main_menu
                .run_if(in_state(AppScreen::Game).or(in_state(AppScreen::Editor))),
        )
        .add_systems(OnExit(AppScreen::Game), cleanup_view::<GameView>)
        .run();
}

fn setup_game_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config: Res<GameConfig>,
) {
    commands.spawn((Camera2d, GameView));
    spawn_demo_cards(&mut commands, &config);

    let ui_font = font::load_assets(asset_server, &config, font::FontType::Default);

    commands.spawn((
        Text::new(config.ui.tutorial_text.clone()),
        TextFont {
            font: ui_font.clone(),
            font_size: config.ui.tutorial_font_size,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(config.ui.tutorial_offset[1]),
            left: px(config.ui.tutorial_offset[0]),
            ..default()
        },
        GameView,
    ));

    commands.spawn((
        Text::new(config.ui.status_initial_text.clone()),
        TextFont {
            font: ui_font,
            font_size: config.ui.status_font_size,
            ..default()
        },
        TextColor(config.ui.status_color()),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(config.ui.status_offset[1]),
            left: px(config.ui.status_offset[0]),
            ..default()
        },
        StatusText,
        GameView,
    ));
}

fn update_status_text(
    config: Res<GameConfig>,
    drag_state: Res<EjectInputState>,
    player_query: Query<&Velocity, With<coin::player::PlayerCoin>>,
    mut text_query: Query<&mut Text, With<StatusText>>,
) {
    let Ok(velocity) = player_query.single() else {
        return;
    };
    let Ok(mut text) = text_query.single_mut() else {
        return;
    };

    let status = if drag_state.charging {
        let charge_ratio = drag_state.eject_vector.length() / config.player.max_eject_distance;
        format!(
            "蓄力中 | 拉距 {:.0}px | 预计平面速度 {:.0}",
            drag_state.eject_vector.length(),
            charge_ratio * config.player.max_planar_speed
        )
    } else if drag_state.aiming {
        format!(
            "待发射 | 当前速度 x:{:.0} y:{:.0} z:{:.0}",
            velocity.x, velocity.y, velocity.z
        )
    } else {
        format!(
            "移动中 | 当前速度 x:{:.0} y:{:.0} z:{:.0}",
            velocity.x, velocity.y, velocity.z
        )
    };

    **text = status;
}
