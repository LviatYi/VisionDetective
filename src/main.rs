pub mod asset;
pub mod coin;
pub mod config;
pub mod physics;
pub mod scene;

use crate::asset::font;
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::{
    EjectInputState, PointerMarker, draw_arena_and_aim, handle_player_eject_input,
    update_aiming_marker, update_player_visuals,
};
use crate::config::GameConfig;
use crate::physics::{Velocity, move_player_coin_transform};
use crate::scene::demo_level::spawn_demo_obstacles;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use physics::obstacle::draw_obstacle_paths;
use physics::vision::{draw_vision_radius, setup_vision_system, update_vision_field_mesh};

#[derive(Component)]
struct StatusText;

#[derive(Resource, Default)]
pub struct CursorWorldPosition(pub Option<Vec2>);

fn main() {
    let config = GameConfig::load();

    App::new()
        .insert_resource(ClearColor(config.window.clear_color()))
        .insert_resource(config.clone())
        .init_resource::<CursorWorldPosition>()
        .init_resource::<EjectInputState>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: config.window.title.clone().into(),
                resolution: WindowResolution::new(config.window.width, config.window.height),
                resizable: config.window.resizable,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                track_cursor_world_position,
                handle_player_eject_input,
                move_player_coin_transform,
                update_vision_field_mesh,
                update_player_visuals,
                update_aiming_marker,
                update_status_text,
                draw_vision_radius,
                draw_obstacle_paths,
                draw_arena_and_aim,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config: Res<GameConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);
    spawn_demo_obstacles(&mut commands, &config);
    setup_vision_system(&mut commands, &config, &mut meshes, &mut materials);

    commands.spawn((
        Mesh2d(meshes.add(Circle::new(config.visuals.player_radius))),
        MeshMaterial2d(materials.add(config.visuals.player_color())),
        Transform::from_translation(Vec3::new(0.0, 0.0, config.visuals.player_z)),
        PlayerCoin::default(),
        Velocity::default(),
    ));

    commands.spawn((
        Mesh2d(meshes.add(Circle::new(config.visuals.pointer_radius))),
        MeshMaterial2d(materials.add(config.visuals.pointer_color())),
        Transform::from_translation(Vec3::new(0.0, 0.0, config.visuals.pointer_z)),
        Visibility::Hidden,
        PointerMarker,
    ));

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
    ));
}

fn track_cursor_world_position(
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut cursor_world: ResMut<CursorWorldPosition>,
) {
    let (Ok(window), Ok((camera, camera_transform))) =
        (window_query.single(), camera_query.single())
    else {
        return;
    };

    cursor_world.0 = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok());
}

fn update_status_text(
    config: Res<GameConfig>,
    drag_state: Res<EjectInputState>,
    player_query: Query<&Velocity, With<PlayerCoin>>,
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
