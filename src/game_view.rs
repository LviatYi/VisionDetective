use crate::AppStatus;
use crate::game_view::main_view::{
    MainMenuView, cleanup_view, handle_main_menu_buttons, setup_main_menu,
    update_main_menu_selection,
};
use bevy::app::{App, Plugin, Update};
use bevy::prelude::*;

pub struct GameViewPlugin;

impl Plugin for GameViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<main_view::MainMenuSelection>();
        app.add_systems(OnEnter(AppStatus::MainMenu), setup_main_menu)
            .add_systems(OnExit(AppStatus::MainMenu), cleanup_view::<MainMenuView>)
            .add_systems(
                Update,
                (handle_main_menu_buttons, update_main_menu_selection)
                    .chain()
                    .run_if(in_state(AppStatus::MainMenu)),
            );
    }
}

pub mod main_view {
    use crate::AppStatus;
    use crate::asset::font;
    use crate::card::Card;
    use crate::config::GameConfig;
    use crate::progress::GameProgress;
    use bevy::app::AppExit;
    use bevy::asset::{AssetServer, Assets, Handle, RenderAssetUsages};
    use bevy::camera::Camera2d;
    use bevy::color::Color;
    use bevy::input::ButtonInput;
    use bevy::math::Quat;
    use bevy::mesh::{Indices, PrimitiveTopology};
    use bevy::prelude::{
        Camera, ColorMaterial, Commands, Component, Entity, Font, GlobalTransform, Justify,
        KeyCode, Mesh, Mesh2d, MeshMaterial2d, MessageWriter, MouseButton, NextState, Pickable,
        Query, Res, ResMut, Resource, State, Text2d, TextColor, TextFont, TextLayout, Time,
        Transform, Vec2, With, Without, default,
    };
    use bevy::sprite::Anchor;
    use bevy::window::{PrimaryWindow, Window};
    #[cfg(all(debug_assertions, feature = "dev-inspector"))]
    use bevy_inspector_egui::bevy_egui::PrimaryEguiContext;

    #[derive(Component)]
    pub struct MainMenuView;

    #[derive(Resource, Default)]
    pub struct MainMenuSelection {
        pending_action: Option<MainMenuAction>,
        elapsed: f32,
    }

    #[derive(Component)]
    pub(super) struct MainMenuButton {
        action: MainMenuAction,
        enabled: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum MainMenuAction {
        NewGame,
        ContinueGame,
        Editor,
        Exit,
    }

    pub(super) fn setup_main_menu(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        config: Res<GameConfig>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<ColorMaterial>>,
        mut selection: ResMut<MainMenuSelection>,
    ) {
        *selection = MainMenuSelection::default();

        let ui_font = font::load_assets(&asset_server, &config, font::FontType::Default);
        let has_save = GameProgress::has_save();

        #[cfg(all(debug_assertions, feature = "dev-inspector"))]
        commands.spawn((
            Camera2d,
            Transform::from_xyz(0.0, 0.0, 0.0),
            PrimaryEguiContext,
            MainMenuView,
        ));

        #[cfg(not(all(debug_assertions, feature = "dev-inspector")))]
        commands.spawn((Camera2d, Transform::from_xyz(0.0, 0.0, 0.0), MainMenuView));

        commands.spawn((
            Mesh2d(meshes.add(rectangle_mesh(MAIN_MENU_BACKGROUND_SIZE))),
            MeshMaterial2d(materials.add(Color::srgba(0.05, 0.06, 0.08, 0.96))),
            Transform::from_xyz(0.0, 0.0, MAIN_MENU_BACKGROUND_Z),
            Pickable::IGNORE,
            MainMenuView,
        ));

        commands.spawn((
            Text2d::new("Vision Detective"),
            TextFont {
                font: ui_font.clone(),
                font_size: 54.0,
                ..default()
            },
            TextColor(Color::WHITE),
            TextLayout::new_with_justify(Justify::Center),
            Anchor::CENTER,
            Transform::from_xyz(0.0, 215.0, MAIN_MENU_TEXT_Z),
            Pickable::IGNORE,
            MainMenuView,
        ));

        let cards = [
            (
                "开始游戏",
                MainMenuAction::NewGame,
                true,
                (-120.0, 60.0, -30.0),
            ),
            (
                "继续游戏",
                MainMenuAction::ContinueGame,
                has_save,
                (-150.0, -150.0, 45.0),
            ),
            ("编辑器", MainMenuAction::Editor, true, (300.0, 100.0, 15.0)),
            ("退出", MainMenuAction::Exit, true, (320.0, -120.0, -25.0)),
        ];
        for (label, action, enabled, (x, y, rotation_angle)) in cards {
            spawn_main_menu_card(
                &mut commands,
                meshes.as_mut(),
                materials.as_mut(),
                &config,
                &ui_font,
                label,
                action,
                enabled,
                x,
                y,
                rotation_angle,
            );
        }
    }

    fn spawn_main_menu_card(
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<ColorMaterial>,
        config: &GameConfig,
        font: &Handle<Font>,
        label: &str,
        action: MainMenuAction,
        enabled: bool,
        x: f32,
        y: f32,
        rotation_angle: f32,
    ) {
        let text_color = if enabled {
            Color::srgb(0.10, 0.08, 0.05)
        } else {
            Color::srgba(0.10, 0.08, 0.05, 0.35)
        };
        let background_color = if enabled {
            Color::srgb(0.88, 0.78, 0.58)
        } else {
            Color::srgb(0.42, 0.39, 0.34)
        };

        commands
            .spawn((
                Mesh2d(meshes.add(Card::card_rounded_mesh(&config.cards))),
                MeshMaterial2d(materials.add(background_color)),
                Transform::from_xyz(x, y, MAIN_MENU_CARD_Z)
                    .with_rotation(Quat::from_rotation_z(rotation_angle.to_radians())),
                Pickable::default(),
                MainMenuButton { action, enabled },
                MainMenuView,
            ))
            .with_children(|card| {
                card.spawn((
                    Text2d::new(label),
                    TextFont {
                        font: font.clone(),
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(text_color),
                    TextLayout::new_with_justify(Justify::Center),
                    Anchor::CENTER,
                    Transform::from_xyz(0.0, 0.0, MAIN_MENU_TEXT_Z_OFFSET),
                    Pickable::IGNORE,
                ));
            });
    }

    pub(super) fn handle_main_menu_buttons(
        mouse_input: Res<ButtonInput<MouseButton>>,
        window_query: Query<&Window, With<PrimaryWindow>>,
        camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
        button_query: Query<(&MainMenuButton, &GlobalTransform)>,
        mut selection: ResMut<MainMenuSelection>,
    ) {
        if selection.pending_action.is_some() {
            return;
        }
        if !mouse_input.just_pressed(MouseButton::Left) {
            return;
        }

        let Ok(window) = window_query.single() else {
            return;
        };
        let Some(cursor_position) = window.cursor_position() else {
            return;
        };
        let Some((camera, camera_transform)) = camera_query
            .iter()
            .filter(|(camera, _)| camera.is_active)
            .max_by_key(|(camera, _)| camera.order)
        else {
            return;
        };
        let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position)
        else {
            return;
        };

        for (button, transform) in &button_query {
            if !button.enabled {
                continue;
            }
            if !main_menu_card_contains_point(transform, world_position) {
                continue;
            }

            selection.pending_action = Some(button.action);
            selection.elapsed = 0.0;
            break;
        }
    }

    fn main_menu_card_contains_point(transform: &GlobalTransform, world_position: Vec2) -> bool {
        let local_position = transform
            .affine()
            .inverse()
            .transform_point3(world_position.extend(0.0))
            .truncate();

        local_position.x.abs() <= Card::HALF_SIZE.x && local_position.y.abs() <= Card::HALF_SIZE.y
    }

    pub(super) fn update_main_menu_selection(
        time: Res<Time>,
        mut selection: ResMut<MainMenuSelection>,
        mut view_query: Query<&mut Transform, (With<MainMenuView>, Without<Camera2d>)>,
        mut progress: ResMut<GameProgress>,
        mut next_screen: ResMut<NextState<AppStatus>>,
        mut exit: MessageWriter<AppExit>,
    ) {
        let Some(action) = selection.pending_action else {
            return;
        };

        selection.elapsed += time.delta_secs();
        let progress_value = (selection.elapsed / MAIN_MENU_EXIT_DURATION).clamp(0.0, 1.0);
        let eased = smoothstep(progress_value);
        for mut transform in &mut view_query {
            transform.translation.x += MAIN_MENU_EXIT_SPEED * eased * time.delta_secs();
        }

        if selection.elapsed < MAIN_MENU_EXIT_DURATION {
            return;
        }

        selection.pending_action = None;
        selection.elapsed = 0.0;

        match action {
            MainMenuAction::NewGame => {
                *progress = GameProgress::default();
                GameProgress::delete_save();
                next_screen.set(AppStatus::Game);
            }
            MainMenuAction::ContinueGame => {
                *progress = GameProgress::load();
                next_screen.set(AppStatus::Game);
            }
            MainMenuAction::Editor => next_screen.set(AppStatus::Editor),
            MainMenuAction::Exit => {
                exit.write(AppExit::Success);
            }
        }
    }

    fn smoothstep(t: f32) -> f32 {
        t * t * (3.0 - 2.0 * t)
    }

    pub fn handle_esc_to_main_menu(
        keyboard_input: Res<ButtonInput<KeyCode>>,
        current_view: Res<State<AppStatus>>,
        mut editor_state: Option<ResMut<crate::editor::EditorInteractionState>>,
        editor_history: Option<Res<crate::editor::EditorUndoHistory>>,
        mut next_screen: ResMut<NextState<AppStatus>>,
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
            if *current_view.get() == AppStatus::Editor {
                let has_unsaved_changes = editor_history
                    .as_ref()
                    .map(|history| history.has_unsaved_changes())
                    .unwrap_or(false);
                if let Some(state) = editor_state.as_mut() {
                    if !state.request_exit_to_main_menu(has_unsaved_changes) {
                        return;
                    }
                }
            }
            next_screen.set(AppStatus::MainMenu);
        }
    }

    pub fn cleanup_view<T: Component>(mut commands: Commands, query: Query<Entity, With<T>>) {
        for entity in &query {
            commands.entity(entity).try_despawn();
        }
    }

    fn rectangle_mesh(size: Vec2) -> Mesh {
        let half = size * 0.5;
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [-half.x, half.y, 0.0],
                [half.x, half.y, 0.0],
                [half.x, -half.y, 0.0],
                [-half.x, -half.y, 0.0],
            ],
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        );
        mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
        mesh
    }

    const MAIN_MENU_EXIT_DURATION: f32 = 0.55;
    const MAIN_MENU_EXIT_SPEED: f32 = 3200.0;
    const MAIN_MENU_BACKGROUND_SIZE: Vec2 = Vec2::new(4000.0, 3000.0);
    const MAIN_MENU_BACKGROUND_Z: f32 = -10.0;
    const MAIN_MENU_CARD_Z: f32 = 0.0;
    const MAIN_MENU_TEXT_Z: f32 = 2.0;
    const MAIN_MENU_TEXT_Z_OFFSET: f32 = 0.3;
}
