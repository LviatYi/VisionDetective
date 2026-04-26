pub mod asset;
pub mod coin;
pub mod obstacle;
pub mod physics;

use crate::asset::font;
use crate::coin::player::controller::{
    EjectInputState, PointerMarker, draw_arena_and_aim, handle_player_eject_input,
    update_aiming_marker, update_player_visuals,
};
use crate::coin::player::{MAX_EJECT_DISTANCE, MAX_PLANAR_SPEED, PlayerCoin};
use crate::obstacle::{draw_obstacle_paths, spawn_demo_obstacles};
use crate::physics::{Velocity, move_player_coin_transform};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 720.0;
const PLAYER_RADIUS: f32 = 28.0;
const POINTER_RADIUS: f32 = 10.0;

#[derive(Component)]
struct StatusText;

#[derive(Resource, Default)]
pub struct CursorWorldPosition(pub Option<Vec2>);

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.06, 0.09, 0.11)))
        .init_resource::<CursorWorldPosition>()
        .init_resource::<EjectInputState>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Vision Detective".into(),
                resolution: WindowResolution::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                resizable: false,
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
                update_player_visuals,
                update_aiming_marker,
                update_status_text,
                draw_obstacle_paths,
                draw_arena_and_aim,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);
    spawn_demo_obstacles(&mut commands);

    commands.spawn((
        Mesh2d(meshes.add(Circle::new(PLAYER_RADIUS))),
        MeshMaterial2d(materials.add(Color::srgb(0.90, 0.84, 0.35))),
        Transform::from_translation(Vec3::new(0.0, 0.0, 2.0)),
        PlayerCoin::default(),
        Velocity::default(),
    ));

    commands.spawn((
        Mesh2d(meshes.add(Circle::new(POINTER_RADIUS))),
        MeshMaterial2d(materials.add(Color::srgb(0.98, 0.43, 0.29))),
        Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
        Visibility::Hidden,
        PointerMarker,
    ));

    let ui_font = font::load_assets(asset_server, font::FontType::Default);

    commands.spawn((
        Text::new("左键按住主角蓄力，松开后朝反方向弹射"),
        TextFont {
            font: ui_font.clone(),
            font_size: 28.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(20),
            left: px(24),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("状态初始化中"),
        TextFont {
            font: ui_font,
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::srgb(0.75, 0.81, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(20),
            left: px(24),
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
        let charge_ratio = drag_state.eject_vector.length() / MAX_EJECT_DISTANCE;
        format!(
            "蓄力中 | 拉距 {:.0}px | 预计平面速度 {:.0}",
            drag_state.eject_vector.length(),
            charge_ratio * MAX_PLANAR_SPEED
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
