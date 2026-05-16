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
        app.init_resource::<main_view::MainMenuBannerState>();
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
        Query, Res, ResMut, Resource, Sprite, State, Text2d, TextColor, TextFont, TextLayout, Time,
        Transform, Vec2, Vec3, With, Without, default,
    };
    use bevy::sprite::Anchor;
    use bevy::window::{PrimaryWindow, Window};
    #[cfg(all(debug_assertions, feature = "dev-inspector"))]
    use bevy_inspector_egui::bevy_egui::PrimaryEguiContext;

    #[derive(Component)]
    pub struct MainMenuView;

    #[derive(Resource, Default)]
    pub struct MainMenuSelection {
        phase: MainMenuPhase,
        pending_action: Option<MainMenuAction>,
        elapsed: f32,
    }

    #[derive(Resource, Default)]
    pub struct MainMenuBannerState {
        shown: bool,
    }

    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
    enum MainMenuPhase {
        #[default]
        Banner,
        Dealing,
        Idle,
        Collecting,
    }

    #[derive(Component)]
    pub(super) struct MainMenuButton {
        action: MainMenuAction,
        enabled: bool,
        delay: f32,
        deal_start: Transform,
        idle_transform: Transform,
    }

    #[derive(Component)]
    pub(super) struct MainMenuTitle {
        start_translation: Vec3,
        idle_translation: Vec3,
    }

    #[derive(Component)]
    pub(super) struct MainMenuBanner;

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
        mut banner_state: ResMut<MainMenuBannerState>,
    ) {
        *selection = MainMenuSelection::default();
        if banner_state.shown {
            selection.phase = MainMenuPhase::Dealing;
        } else {
            banner_state.shown = true;
        }

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
            Sprite {
                image: asset_server.load("pic/built-with-rust-bevy-banner.png"),
                custom_size: Some(MAIN_MENU_BANNER_SIZE),
                color: Color::srgba(1.0, 1.0, 1.0, 0.0),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, MAIN_MENU_BANNER_Z),
            Pickable::IGNORE,
            MainMenuBanner,
            MainMenuView,
        ));

        commands.spawn((
            Mesh2d(meshes.add(rectangle_mesh(MAIN_MENU_BACKGROUND_SIZE))),
            MeshMaterial2d(materials.add(config.window.clear_color())),
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
            Transform::from_translation(MAIN_MENU_TITLE_START),
            Pickable::IGNORE,
            MainMenuTitle {
                start_translation: MAIN_MENU_TITLE_START,
                idle_translation: MAIN_MENU_TITLE_IDLE,
            },
            MainMenuView,
        ));

        let cards = [
            (
                "开始游戏",
                MainMenuAction::NewGame,
                true,
                (210.0, 60.0, -30.0),
            ),
            (
                "继续游戏",
                MainMenuAction::ContinueGame,
                has_save,
                (180.0, -150.0, 45.0),
            ),
            ("编辑器", MainMenuAction::Editor, true, (380.0, 100.0, 15.0)),
            ("退出", MainMenuAction::Exit, true, (440.0, -120.0, -25.0)),
        ];
        for (index, (label, action, enabled, (x, y, rotation_angle))) in
            cards.into_iter().enumerate()
        {
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
                index as f32 * MAIN_MENU_CARD_DEAL_INTERVAL,
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
        delay: f32,
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

        let idle_transform = Transform::from_xyz(x, y, MAIN_MENU_CARD_Z)
            .with_rotation(Quat::from_rotation_z(rotation_angle.to_radians()));
        let deal_start = Transform::from_xyz(
            MAIN_MENU_CARD_DECK_POSITION.x,
            MAIN_MENU_CARD_DECK_POSITION.y,
            MAIN_MENU_CARD_Z,
        );

        commands
            .spawn((
                Mesh2d(meshes.add(Card::card_rounded_mesh(&config.cards))),
                MeshMaterial2d(materials.add(background_color)),
                deal_start,
                Pickable::default(),
                MainMenuButton {
                    action,
                    enabled,
                    delay,
                    deal_start,
                    idle_transform,
                },
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
        if selection.phase != MainMenuPhase::Idle {
            return;
        }
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
            selection.phase = MainMenuPhase::Collecting;
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
        mut title_query: Query<(&MainMenuTitle, &mut Transform), Without<MainMenuButton>>,
        mut button_query: Query<(&MainMenuButton, &mut Transform), Without<MainMenuTitle>>,
        mut banner_query: Query<
            (&mut Sprite, &mut Transform),
            (
                With<MainMenuBanner>,
                Without<MainMenuTitle>,
                Without<MainMenuButton>,
            ),
        >,
        mut progress: ResMut<GameProgress>,
        mut next_screen: ResMut<NextState<AppStatus>>,
        mut exit: MessageWriter<AppExit>,
    ) {
        selection.elapsed += time.delta_secs();

        match selection.phase {
            MainMenuPhase::Banner => {
                let alpha = banner_alpha(selection.elapsed);
                for (mut sprite, mut transform) in &mut banner_query {
                    sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha);
                    transform.scale = Vec3::splat(
                        1.0 + 0.025
                            * smoothstep(
                                (selection.elapsed / MAIN_MENU_BANNER_DURATION).clamp(0.0, 1.0),
                            ),
                    );
                }

                if selection.elapsed >= MAIN_MENU_BANNER_DURATION {
                    selection.phase = MainMenuPhase::Dealing;
                    selection.elapsed = 0.0;
                }
            }
            MainMenuPhase::Dealing => {
                let title_progress =
                    (selection.elapsed / MAIN_MENU_TITLE_DEAL_DURATION).clamp(0.0, 1.0);
                let title_eased = smoothstep(title_progress);
                for (title, mut transform) in &mut title_query {
                    transform.translation = title
                        .start_translation
                        .lerp(title.idle_translation, title_eased);
                }

                let mut all_done = title_progress >= 1.0;
                for (button, mut transform) in &mut button_query {
                    let card_progress = ((selection.elapsed - button.delay)
                        / MAIN_MENU_CARD_DEAL_DURATION)
                        .clamp(0.0, 1.0);
                    if card_progress < 1.0 {
                        all_done = false;
                    }
                    let card_eased = smoothstep(card_progress);
                    transform.translation = button
                        .deal_start
                        .translation
                        .lerp(button.idle_transform.translation, card_eased);
                    transform.rotation = button
                        .deal_start
                        .rotation
                        .slerp(button.idle_transform.rotation, card_eased);
                }

                if all_done {
                    selection.phase = MainMenuPhase::Idle;
                    selection.elapsed = 0.0;
                }
            }
            MainMenuPhase::Idle => {}
            MainMenuPhase::Collecting => {
                let progress_value =
                    (selection.elapsed / MAIN_MENU_COLLECT_DURATION).clamp(0.0, 1.0);
                let eased = smoothstep(progress_value);
                for (title, mut transform) in &mut title_query {
                    transform.translation =
                        title.idle_translation.lerp(title.start_translation, eased);
                }
                for (button, mut transform) in &mut button_query {
                    transform.translation = button
                        .idle_transform
                        .translation
                        .lerp(button.deal_start.translation, eased);
                    transform.rotation = button
                        .idle_transform
                        .rotation
                        .slerp(button.deal_start.rotation, eased);
                }

                if selection.elapsed < MAIN_MENU_COLLECT_DURATION {
                    return;
                }

                let Some(action) = selection.pending_action else {
                    return;
                };
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
        }
    }

    fn banner_alpha(elapsed: f32) -> f32 {
        if elapsed < MAIN_MENU_BANNER_FADE_IN {
            smoothstep((elapsed / MAIN_MENU_BANNER_FADE_IN).clamp(0.0, 1.0))
        } else if elapsed > MAIN_MENU_BANNER_DURATION - MAIN_MENU_BANNER_FADE_OUT {
            1.0 - smoothstep(
                ((elapsed - (MAIN_MENU_BANNER_DURATION - MAIN_MENU_BANNER_FADE_OUT))
                    / MAIN_MENU_BANNER_FADE_OUT)
                    .clamp(0.0, 1.0),
            )
        } else {
            1.0
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

    const MAIN_MENU_BACKGROUND_SIZE: Vec2 = Vec2::new(4000.0, 3000.0);
    const MAIN_MENU_BACKGROUND_Z: f32 = -10.0;
    const MAIN_MENU_BANNER_SIZE: Vec2 = Vec2::new(1920.0, 1080.0);
    const MAIN_MENU_BANNER_Z: f32 = 4.0;
    const MAIN_MENU_CARD_Z: f32 = 0.0;
    const MAIN_MENU_TEXT_Z: f32 = 2.0;
    const MAIN_MENU_TEXT_Z_OFFSET: f32 = 0.3;
    const MAIN_MENU_BANNER_DURATION: f32 = 5.0;
    const MAIN_MENU_BANNER_FADE_IN: f32 = 0.35;
    const MAIN_MENU_BANNER_FADE_OUT: f32 = 0.45;
    const MAIN_MENU_TITLE_START: Vec3 = Vec3::new(-1920.0, 245.0, MAIN_MENU_TEXT_Z);
    const MAIN_MENU_TITLE_IDLE: Vec3 = Vec3::new(-300.0, 245.0, MAIN_MENU_TEXT_Z);
    const MAIN_MENU_TITLE_DEAL_DURATION: f32 = 0.75;
    const MAIN_MENU_CARD_DECK_POSITION: Vec2 = Vec2::new(760.0, -80.0);
    const MAIN_MENU_CARD_DEAL_DURATION: f32 = 0.52;
    const MAIN_MENU_CARD_DEAL_INTERVAL: f32 = 0.12;
    const MAIN_MENU_COLLECT_DURATION: f32 = 0.55;
}
