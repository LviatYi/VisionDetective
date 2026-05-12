pub mod asset;
pub mod camera_control;
pub mod card;
pub mod coin;
pub mod config;
pub mod editor;
mod game_view;
pub mod input;
pub mod physics;
pub mod picking;
pub mod progress;
pub mod scene;
pub mod tools;

use crate::asset::font;
use crate::camera_control::{CameraControlPlugin, GameCamera};
use crate::card::CardPlugin;
use crate::card::card_params::CardSpawnParams;
use crate::coin::player::PlayerPlugin;
use crate::coin::player::controller::{PlayerCoinBehaviorState, PlayerCoinState};
use crate::config::GameConfig;
use crate::config::card_config::CardPresetsConfig;
use crate::editor::EditorPlugin;
use crate::game_view::main_view::{cleanup_view, handle_esc_to_main_menu};
use crate::game_view::{AppView, GameState, GameViewPlugin, GameplaySet};
use crate::input::GameplayInputBlocker;
use crate::physics::PhysicsPlugin;
use crate::physics::Velocity;
use crate::physics::vision::VisionPlugin;
use crate::picking::VisionPickingPlugin;
use crate::progress::{GameProgress, GameProgressPlugin};
use crate::scene::demo_level::RuntimeScenePlugin;
use crate::scene::demo_level::load_demo_scene;
use crate::scene::get_layered_game_scene_camera2d_bundle;
use bevy::prelude::*;
use bevy::window::WindowResolution;

#[derive(Component)]
pub struct GameView;

#[derive(Component)]
struct StatusText;

fn main() {
    let config = GameConfig::load();
    let card_presets_config = CardPresetsConfig::load();

    App::new()
        .insert_resource(ClearColor(config.window.clear_color()))
        .insert_resource(config.clone())
        .insert_resource(card_presets_config.clone())
        .init_resource::<GameplayInputBlocker>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: config.window.title.clone().into(),
                resolution: WindowResolution::new(config.window.width, config.window.height),
                resizable: config.window.resizable,
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppView>()
        .add_sub_state::<GameState>()
        .add_plugins((
            GameViewPlugin,
            PlayerPlugin,
            PhysicsPlugin,
            VisionPlugin,
            VisionPickingPlugin,
            CardPlugin,
            EditorPlugin,
            CameraControlPlugin,
            GameProgressPlugin,
            RuntimeScenePlugin,
        ))
        .add_systems(OnEnter(GameState::Loading), setup_game_scene)
        .add_systems(
            Update,
            finish_game_loading.run_if(in_state(GameState::Loading)),
        )
        .add_systems(Update, update_status_text.in_set(GameplaySet::Visual))
        .add_systems(
            Update,
            handle_esc_to_main_menu
                .after(editor::cancel_prefab_drag_with_escape)
                .run_if(in_state(AppView::Game).or(in_state(AppView::Editor))),
        )
        .add_systems(OnExit(AppView::Game), cleanup_view::<GameView>)
        .run();
}

fn finish_game_loading(mut next_game_state: ResMut<NextState<GameState>>) {
    next_game_state.set(GameState::InGame);
}

fn setup_game_scene(
    mut commands: Commands,
    mut card_spawn_params: CardSpawnParams<'_>,
    progress: Res<GameProgress>,
    mut scene_cards: ResMut<scene::demo_level::RuntimeSceneCards>,
) {
    commands.spawn((
        get_layered_game_scene_camera2d_bundle(),
        GameView,
        GameCamera,
    ));

    load_demo_scene(
        &mut commands,
        &mut card_spawn_params,
        &progress,
        &mut scene_cards,
    );

    let ui_font = font::load_assets(
        &card_spawn_params.asset_server,
        &card_spawn_params.config,
        font::FontType::Default,
    );

    commands.spawn((
        Text::new(card_spawn_params.config.ui.tutorial_text.clone()),
        TextFont {
            font: ui_font.clone(),
            font_size: card_spawn_params.config.ui.tutorial_font_size,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(card_spawn_params.config.ui.tutorial_offset[1]),
            left: px(card_spawn_params.config.ui.tutorial_offset[0]),
            ..default()
        },
        GameView,
    ));

    commands.spawn((
        Text::new(card_spawn_params.config.ui.status_initial_text.clone()),
        TextFont {
            font: ui_font,
            font_size: card_spawn_params.config.ui.status_font_size,
            ..default()
        },
        TextColor(card_spawn_params.config.ui.status_color()),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(card_spawn_params.config.ui.status_offset[1]),
            left: px(card_spawn_params.config.ui.status_offset[0]),
            ..default()
        },
        StatusText,
        GameView,
    ));
}

fn update_status_text(
    config: Res<GameConfig>,
    player_state: Res<PlayerCoinState>,
    player_query: Query<&Velocity, With<coin::player::PlayerCoin>>,
    mut text_query: Query<&mut Text, With<StatusText>>,
) {
    let Ok(velocity) = player_query.single() else {
        return;
    };
    let Ok(mut text) = text_query.single_mut() else {
        return;
    };

    let status = if let PlayerCoinBehaviorState::Charging { eject_vector } = **player_state {
        let charge_ratio = eject_vector.length() / config.player.max_eject_distance;
        format!(
            "蓄力中 | 拉距 {:.0}px | 预计平面速度 {:.0}",
            eject_vector.length(),
            charge_ratio * config.player.max_planar_speed
        )
    } else if player_state.is_aiming() {
        format!(
            "待发射 | 当前速度 x:{:.0} y:{:.0} z:{:.0}",
            velocity.x, velocity.y, velocity.z
        )
    } else if player_state.is_idle() {
        format!(
            "静止 | 当前速度 x:{:.0} y:{:.0} z:{:.0}",
            velocity.x, velocity.y, velocity.z
        )
    } else {
        format!(
            "弹起中 | 当前速度 x:{:.0} y:{:.0} z:{:.0}",
            velocity.x, velocity.y, velocity.z
        )
    };

    **text = status;
}
