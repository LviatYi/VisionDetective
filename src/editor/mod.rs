use crate::AppScreen;
use crate::card::card_params::{CardParam, CardSceneParam, CardSpecializedRegistry};
use crate::card::{CARD_SIZE, Card, spawn_card_by_card_param};
use crate::config::GameConfig;
use crate::config::card_config::CardPresetsConfig;
use crate::editor::editor_view::{EditorView, setup_editor_view};
use crate::game_view::main_view::cleanup_view;
use bevy::ecs::system::SystemParam;
use bevy::input::ButtonInput;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::f32::consts::FRAC_PI_4;
use std::fs;
use std::path::{Path, PathBuf};

const SIDEBAR_WIDTH: f32 = 264.0;
const TOOLBAR_HEIGHT: f32 = 48.0;
const COMPACT_BUTTON_HEIGHT: f32 = 30.0;
const PREFAB_LIST_HEIGHT: f32 = 252.0;
const PREFAB_CARD_HEIGHT: f32 = 76.0;
const PREFAB_CARD_GAP: f32 = 12.0;
const PREFAB_SCROLL_STEP: f32 = 28.0;
const ROTATION_HANDLE_RADIUS: f32 = 14.0;
const EDITOR_SCENE_DIR: &str = "assets/editor";
const EDITOR_SCENE_TOML: &str = "editor_scene.toml";
const EDITOR_SCENE_BIN: &str = "editor_scene.bin";

pub struct EditorPlugin;

#[derive(Resource, Default)]
pub struct EditorInteractionState {
    selected_entity: Option<Entity>,
    dragging_prefab: Option<u32>,
    drag_preview_entity: Option<Entity>,
    moving_entity: Option<MovingEntityState>,
    rotating_entity: Option<RotatingEntityState>,
    prefab_scroll_offset: f32,
    escape_consumed: bool,
    status_message: String,
}

impl EditorInteractionState {
    pub fn captures_pointer(&self) -> bool {
        self.dragging_prefab.is_some()
            || self.moving_entity.is_some()
            || self.rotating_entity.is_some()
    }

    pub fn take_escape_consumed(&mut self) -> bool {
        let consumed = self.escape_consumed;
        self.escape_consumed = false;
        consumed
    }
}

#[derive(Bundle)]
struct EditorButtonBundle {
    button: Button,
    node: Node,
    background_color: BackgroundColor,
    action: EditorButtonAction,
}

#[derive(Clone, Copy)]
pub struct MovingEntityState {
    entity: Entity,
    pointer_offset: Vec2,
}

#[derive(Clone, Copy)]
pub struct RotatingEntityState {
    entity: Entity,
}

#[derive(Component)]
struct EditorStatusText;

#[derive(Component, Clone, Copy)]
enum EditorButtonAction {
    ExportScene,
    ImportScene,
}

#[derive(Component, Clone, Copy)]
struct PrefabPreviewButton {
    prefab_id: u32,
}

#[derive(Component)]
struct PrefabListContent;

#[derive(Component)]
struct EditorDragPreview;

struct PrefabPreviewItem {
    id: u32,
    title: String,
    background_color: Color,
}

#[derive(SystemParam)]
struct EditorCardSpawnDeps<'w> {
    asset_server: Res<'w, AssetServer>,
    config: Res<'w, GameConfig>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
    card_presets_config: Res<'w, CardPresetsConfig>,
    card_specialized_registry: Res<'w, CardSpecializedRegistry>,
}

#[derive(Serialize, Deserialize, Default)]
struct EditorSceneFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<CardParam>,
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorInteractionState>()
            .add_systems(OnEnter(AppScreen::Editor), reset_editor_state)
            .add_systems(OnEnter(AppScreen::Editor), setup_editor_view)
            .add_systems(OnEnter(AppScreen::Editor), setup_editor_ui)
            .add_systems(OnExit(AppScreen::Editor), cleanup_view::<EditorView>)
            .add_systems(
                Update,
                (
                    handle_toolbar_buttons,
                    handle_prefab_drag_start,
                    handle_prefab_list_scroll,
                    update_drag_preview_card,
                    cancel_prefab_drag_with_escape,
                    handle_prefab_drop,
                    handle_scene_editing,
                    handle_editor_shortcuts,
                    update_editor_status_text,
                )
                    .run_if(in_state(AppScreen::Editor)),
            )
            .add_systems(
                Update,
                draw_editor_gizmos.run_if(in_state(AppScreen::Editor)),
            );
    }
}

fn reset_editor_state(mut state: ResMut<EditorInteractionState>) {
    *state = EditorInteractionState {
        status_message: "编辑器已就绪".into(),
        ..default()
    };
}

fn setup_editor_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config: Res<GameConfig>,
    card_presets_config: Res<CardPresetsConfig>,
) {
    let ui_font = asset_server.load(config.assets.default_font.clone());
    let preview_items = prefab_preview_items(&card_presets_config, &config);

    commands
        .spawn((
            Node {
                width: percent(100.0),
                height: percent(100.0),
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::NONE),
            EditorView,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: percent(100.0),
                        height: px(TOOLBAR_HEIGHT),
                        justify_content: JustifyContent::Start,
                        align_items: AlignItems::Center,
                        column_gap: px(6.0),
                        padding: UiRect::axes(px(10.0), px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.07, 0.09, 0.12, 0.96)),
                ))
                .with_children(|toolbar| {
                    spawn_button(toolbar, &ui_font, "导出", EditorButtonAction::ExportScene);
                    spawn_button(toolbar, &ui_font, "导入", EditorButtonAction::ImportScene);
                });

            parent
                .spawn((
                    Node {
                        width: px(SIDEBAR_WIDTH),
                        height: percent(100.0),
                        position_type: PositionType::Absolute,
                        top: px(TOOLBAR_HEIGHT),
                        left: px(0.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(12.0),
                        padding: UiRect::all(px(16.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.10, 0.11, 0.15, 0.35)),
                ))
                .with_children(|sidebar| {
                    sidebar.spawn((
                        Text::new("卡牌预制体"),
                        TextFont {
                            font: ui_font.clone(),
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));

                    sidebar
                        .spawn((
                            Node {
                                width: percent(100.0),
                                height: px(PREFAB_LIST_HEIGHT),
                                position_type: PositionType::Relative,
                                overflow: Overflow::clip_y(),
                                padding: UiRect::all(px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.14, 0.16, 0.20, 0.72)),
                        ))
                        .with_children(|viewport| {
                            viewport
                                .spawn((
                                    Node {
                                        width: percent(100.0),
                                        position_type: PositionType::Absolute,
                                        top: px(0.0),
                                        left: px(0.0),
                                        flex_direction: FlexDirection::Column,
                                        row_gap: px(PREFAB_CARD_GAP),
                                        padding: UiRect::all(px(8.0)),
                                        ..default()
                                    },
                                    PrefabListContent,
                                ))
                                .with_children(|list| {
                                    for item in &preview_items {
                                        spawn_prefab_preview_items(
                                            list,
                                            &ui_font,
                                            &item.title,
                                            item.id,
                                            item.background_color,
                                        );
                                    }
                                });
                        });

                    sidebar.spawn((
                        Text::new(
                            "操作说明\n1. 直接按住卡牌预览并拖到主场景，松开后会克隆一张。\n2. 左键拖动卡牌位置。\n3. 拖动右上角旋转控制点可直接旋转。\n4. Delete 删除选中卡牌。\n5. Ctrl+E / Ctrl+B 导出，Ctrl+I / Ctrl+Shift+I 导入。",
                        ),
                        TextFont {
                            font: ui_font.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.78, 0.82, 0.88)),
                    ));

                    sidebar.spawn((
                        Text::new("编辑器已就绪"),
                        TextFont {
                            font: ui_font,
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.62, 0.87, 0.72)),
                        EditorStatusText,
                    ));
                });
        });
}

fn prefab_preview_items(
    card_presets_config: &CardPresetsConfig,
    config: &GameConfig,
) -> Vec<PrefabPreviewItem> {
    card_presets_config
        .prefabs
        .iter()
        .map(|prefab| {
            let card_param = CardParam {
                scene_param: CardSceneParam {
                    position: Vec2::ZERO,
                    rotation: 0.0,
                },
                prefab_id: prefab.id,
            };
            let appearance = card_param.load_appearance(card_presets_config);
            let background_color = parse_ui_color(&appearance.background_color_appearance_override)
                .unwrap_or_else(|| config.cards.default_fill_color());

            PrefabPreviewItem {
                id: prefab.id,
                title: appearance.title,
                background_color,
            }
        })
        .collect()
}

fn spawn_button(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: &str,
    action: EditorButtonAction,
) {
    parent
        .spawn(EditorButtonBundle {
            button: Button,
            node: Node {
                height: px(COMPACT_BUTTON_HEIGHT),
                padding: UiRect::axes(px(10.0), px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            background_color: BackgroundColor(Color::srgb(0.19, 0.29, 0.40)),
            action,
        })
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: font.clone(),
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn spawn_prefab_preview_items(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    title: &str,
    prefab_id: u32,
    background_color: Color,
) {
    parent
        .spawn((
            Button,
            Node {
                width: percent(100.0),
                min_height: px(PREFAB_CARD_HEIGHT),
                padding: UiRect::all(px(10.0)),
                column_gap: px(10.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(background_color),
            PrefabPreviewButton { prefab_id },
        ))
        .with_children(|button| {
            button
                .spawn((Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4.0),
                    ..default()
                },))
                .with_children(|text_wrap| {
                    text_wrap.spawn((
                        Text::new(title.to_string()),
                        TextFont {
                            font: font.clone(),
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    text_wrap.spawn((
                        Text::new(format!("prefab_id: {prefab_id}")),
                        TextFont {
                            font: font.clone(),
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.72, 0.78, 0.86)),
                    ));
                });
        });
}

fn handle_toolbar_buttons(
    mut commands: Commands,
    interaction_query: Query<(&Interaction, &EditorButtonAction), Changed<Interaction>>,
    card_query: Query<(Entity, &Card, &Transform), Without<EditorDragPreview>>,
    mut spawn_deps: EditorCardSpawnDeps,
    mut state: ResMut<EditorInteractionState>,
) {
    for (interaction, action) in &interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            EditorButtonAction::ExportScene => {
                state.status_message = match pick_scene_export_path() {
                    Some(path) => save_scene_to_path(&card_query, &path),
                    None => "已取消导出".into(),
                };
            }
            EditorButtonAction::ImportScene => {
                state.status_message = match pick_scene_import_path() {
                    Some(path) => {
                        load_scene_from_path(&mut commands, &card_query, &mut spawn_deps, &path)
                    }
                    None => "已取消导入".into(),
                };
                state.selected_entity = None;
            }
        }
    }
}

fn handle_prefab_drag_start(
    mut commands: Commands,
    interaction_query: Query<(&Interaction, &PrefabPreviewButton), Changed<Interaction>>,
    mut spawn_deps: EditorCardSpawnDeps,
    mut state: ResMut<EditorInteractionState>,
) {
    for (interaction, preview_button) in &interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }

        if let Some(entity) = state.drag_preview_entity.take() {
            commands.entity(entity).despawn();
        }

        let entity = spawn_editor_card(
            &mut commands,
            &mut spawn_deps,
            &CardParam {
                scene_param: CardSceneParam {
                    position: Vec2::ZERO,
                    rotation: 0.0,
                },
                prefab_id: preview_button.prefab_id,
            },
        );
        commands
            .entity(entity)
            .insert((EditorDragPreview, Visibility::Hidden));

        state.dragging_prefab = Some(preview_button.prefab_id);
        state.drag_preview_entity = Some(entity);
        state.moving_entity = None;
        state.rotating_entity = None;
        state.status_message = format!("正在拖拽预制体 #{}", preview_button.prefab_id);
    }
}

fn handle_prefab_list_scroll(
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    prefab_query: Query<&PrefabPreviewButton>,
    mut content_query: Query<&mut Node, With<PrefabListContent>>,
    mut state: ResMut<EditorInteractionState>,
) {
    let scroll_units: f32 = mouse_wheel_events.read().map(|event| event.y).sum();
    if scroll_units.abs() <= f32::EPSILON {
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    if !cursor_is_over_sidebar(window, cursor_position) {
        return;
    }

    let item_count = prefab_query.iter().count();
    let content_height = item_count as f32 * PREFAB_CARD_HEIGHT
        + item_count.saturating_sub(1) as f32 * PREFAB_CARD_GAP
        + 16.0;
    let max_offset = (content_height - PREFAB_LIST_HEIGHT).max(0.0);
    state.prefab_scroll_offset =
        (state.prefab_scroll_offset - scroll_units * PREFAB_SCROLL_STEP).clamp(0.0, max_offset);

    let Ok(mut content_node) = content_query.single_mut() else {
        return;
    };
    content_node.top = px(-state.prefab_scroll_offset);
}

fn update_drag_preview_card(
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    state: Res<EditorInteractionState>,
    mut preview_query: Query<(&mut Transform, &mut Visibility), With<EditorDragPreview>>,
) {
    let Some(preview_entity) = state.drag_preview_entity else {
        return;
    };

    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    let Some(world_position) = cursor_world_position(&camera_query, cursor_position) else {
        return;
    };

    let Ok((mut transform, mut visibility)) = preview_query.get_mut(preview_entity) else {
        return;
    };

    transform.translation.x = world_position.x;
    transform.translation.y = world_position.y;
    transform.translation.z = 20.0;
    transform.rotation = Quat::IDENTITY;
    *visibility = Visibility::Visible;
}

pub(crate) fn cancel_prefab_drag_with_escape(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorInteractionState>,
) {
    if state.dragging_prefab.is_none() || !keyboard_input.just_pressed(KeyCode::Escape) {
        return;
    }

    if let Some(entity) = state.drag_preview_entity.take() {
        commands.entity(entity).despawn();
    }
    state.dragging_prefab = None;
    state.escape_consumed = true;
    state.status_message = "已取消拖拽预制体".into();
}

fn handle_prefab_drop(
    mut commands: Commands,
    mouse_input: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut preview_query: Query<&mut Transform, With<EditorDragPreview>>,
    mut state: ResMut<EditorInteractionState>,
) {
    let Some(prefab_id) = state.dragging_prefab else {
        return;
    };
    if !mouse_input.just_released(MouseButton::Left) {
        return;
    }

    state.dragging_prefab = None;
    let preview_entity = state.drag_preview_entity.take();

    let Ok(window) = window_query.single() else {
        if let Some(entity) = preview_entity {
            commands.entity(entity).despawn();
        }
        state.status_message = "放置卡牌失败：窗口不可用".into();
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        if let Some(entity) = preview_entity {
            commands.entity(entity).despawn();
        }
        state.status_message = "放置卡牌失败：光标位置不可用".into();
        return;
    };

    if !cursor_is_over_scene(window, cursor_position) {
        if let Some(entity) = preview_entity {
            commands.entity(entity).despawn();
        }
        state.status_message = "已取消放置：请在主场景区域松开鼠标".into();
        return;
    }

    let Some(world_position) = cursor_world_position(&camera_query, cursor_position) else {
        if let Some(entity) = preview_entity {
            commands.entity(entity).despawn();
        }
        state.status_message = "放置卡牌失败：无法映射到场景坐标".into();
        return;
    };

    let Some(entity) = preview_entity else {
        state.status_message = "放置卡牌失败：拖拽预览不存在".into();
        return;
    };

    if let Ok(mut transform) = preview_query.get_mut(entity) {
        transform.translation.x = world_position.x;
        transform.translation.y = world_position.y;
        transform.translation.z = 0.0;
        transform.rotation = Quat::IDENTITY;
    }

    commands
        .entity(entity)
        .remove::<EditorDragPreview>()
        .insert(Visibility::Visible);
    state.selected_entity = Some(entity);
    state.status_message = format!(
        "已放置卡牌 #{} ({:.0}, {:.0})",
        prefab_id, world_position.x, world_position.y
    );
}

fn handle_scene_editing(
    mut commands: Commands,
    mouse_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut card_queries: ParamSet<(
        Query<(Entity, &Card, &GlobalTransform, &Transform), Without<EditorDragPreview>>,
        Query<&mut Transform>,
    )>,
    mut state: ResMut<EditorInteractionState>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    if !cursor_is_over_scene(window, cursor_position) {
        if mouse_input.just_released(MouseButton::Left) {
            state.moving_entity = None;
            state.rotating_entity = None;
        }
        return;
    }

    let Some(cursor_world_position) = cursor_world_position(&camera_query, cursor_position) else {
        return;
    };

    if mouse_input.just_pressed(MouseButton::Left) {
        {
            let card_query = card_queries.p0();

            if let Some(selected_entity) = state.selected_entity
                && rotation_handle_contains_point(
                    selected_entity,
                    cursor_world_position,
                    &card_query,
                )
            {
                state.rotating_entity = Some(RotatingEntityState {
                    entity: selected_entity,
                });
                state.moving_entity = None;
                state.status_message = "正在旋转卡牌".into();
                return;
            }

            if let Some((entity, transform)) = pick_card(cursor_world_position, &card_query) {
                state.selected_entity = Some(entity);
                state.rotating_entity = None;
                state.moving_entity = Some(MovingEntityState {
                    entity,
                    pointer_offset: transform.translation.truncate() - cursor_world_position,
                });
                state.status_message = format!("已选中卡牌 #{entity:?}，拖动中");
            } else if !keyboard_input.pressed(KeyCode::ControlLeft)
                && !keyboard_input.pressed(KeyCode::ControlRight)
            {
                state.selected_entity = None;
                state.status_message = "已取消选中".into();
            }
        }
    }

    if mouse_input.pressed(MouseButton::Left) {
        let mut transform_query = card_queries.p1();

        if let Some(moving) = state.moving_entity
            && let Ok(mut transform) = transform_query.get_mut(moving.entity)
        {
            let next = cursor_world_position + moving.pointer_offset;
            transform.translation.x = next.x;
            transform.translation.y = next.y;
        }

        if let Some(rotating) = state.rotating_entity
            && let Ok(mut transform) = transform_query.get_mut(rotating.entity)
        {
            let center = transform.translation.truncate();
            let angle = (cursor_world_position - center).to_angle() - FRAC_PI_4;
            transform.rotation = Quat::from_rotation_z(angle);
        }
    }

    if mouse_input.just_released(MouseButton::Left) {
        if state.moving_entity.is_some() {
            state.status_message = "卡牌位置已更新".into();
        } else if state.rotating_entity.is_some() {
            state.status_message = "卡牌旋转已更新".into();
        }
        state.moving_entity = None;
        state.rotating_entity = None;
    }

    if keyboard_input.just_pressed(KeyCode::Delete)
        && let Some(entity) = state.selected_entity.take()
    {
        commands.entity(entity).despawn();
        state.moving_entity = None;
        state.rotating_entity = None;
        state.status_message = "已删除选中卡牌".into();
    }
}

fn handle_editor_shortcuts(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    card_query: Query<(Entity, &Card, &Transform), Without<EditorDragPreview>>,
    spawn_deps: EditorCardSpawnDeps,
    mut state: ResMut<EditorInteractionState>,
) {
    let ctrl_pressed = keyboard_input.pressed(KeyCode::ControlLeft)
        || keyboard_input.pressed(KeyCode::ControlRight);

    if !ctrl_pressed {
        return;
    }

    if keyboard_input.just_pressed(KeyCode::KeyE) {
        state.status_message = save_scene(&card_query, SceneFileFormat::Toml);
    }

    if keyboard_input.just_pressed(KeyCode::KeyB) {
        state.status_message = save_scene(&card_query, SceneFileFormat::Binary);
    }

    if keyboard_input.just_pressed(KeyCode::KeyI) {
        let format = if keyboard_input.pressed(KeyCode::ShiftLeft)
            || keyboard_input.pressed(KeyCode::ShiftRight)
        {
            SceneFileFormat::Binary
        } else {
            SceneFileFormat::Toml
        };
        state.status_message = load_scene(&mut commands, &card_query, spawn_deps, format);
        state.selected_entity = None;
    }
}

fn update_editor_status_text(
    state: Res<EditorInteractionState>,
    mut text_query: Query<&mut Text, With<EditorStatusText>>,
) {
    if !state.is_changed() {
        return;
    }

    let Ok(mut text) = text_query.single_mut() else {
        return;
    };

    let selection_text = match state.selected_entity {
        Some(entity) => format!("当前选中: #{entity:?}"),
        None => "当前选中: 无".into(),
    };

    **text = format!("{selection_text}\n{}", state.status_message);
}

fn draw_editor_gizmos(
    mut gizmos: Gizmos,
    state: Res<EditorInteractionState>,
    card_query: Query<(Entity, &Transform), (With<Card>, Without<EditorDragPreview>)>,
) {
    if let Some(selected_entity) = state.selected_entity
        && let Ok((_, transform)) = card_query.get(selected_entity)
    {
        draw_card_outline(&mut gizmos, transform, Color::srgb(0.32, 0.90, 0.95));
        let handle = rotation_handle_position(transform);
        gizmos.circle_2d(
            handle,
            ROTATION_HANDLE_RADIUS,
            Color::srgb(0.95, 0.47, 0.24),
        );
        gizmos.line_2d(
            transform.translation.truncate(),
            handle,
            Color::srgb(0.95, 0.47, 0.24),
        );
    }
}

fn draw_card_outline(gizmos: &mut Gizmos, transform: &Transform, color: Color) {
    let corners = obstacle_card_corners(transform);
    for index in 0..corners.len() {
        gizmos.line_2d(corners[index], corners[(index + 1) % corners.len()], color);
    }
}

fn obstacle_card_corners(transform: &Transform) -> [Vec2; 4] {
    let half = CARD_SIZE * 0.5;
    let matrix = transform.to_matrix();
    [
        matrix
            .transform_point3(Vec3::new(-half.x, -half.y, 0.0))
            .truncate(),
        matrix
            .transform_point3(Vec3::new(half.x, -half.y, 0.0))
            .truncate(),
        matrix
            .transform_point3(Vec3::new(half.x, half.y, 0.0))
            .truncate(),
        matrix
            .transform_point3(Vec3::new(-half.x, half.y, 0.0))
            .truncate(),
    ]
}

fn rotation_handle_position(transform: &Transform) -> Vec2 {
    let half = CARD_SIZE * 0.5;
    let local = Vec3::new(
        half.x + ROTATION_HANDLE_RADIUS * 1.5,
        half.y + ROTATION_HANDLE_RADIUS * 1.5,
        0.0,
    );
    transform.to_matrix().transform_point3(local).truncate()
}

fn rotation_handle_contains_point<F: bevy::ecs::query::QueryFilter>(
    entity: Entity,
    cursor_world: Vec2,
    card_query: &Query<(Entity, &Card, &GlobalTransform, &Transform), F>,
) -> bool {
    let Ok((_, _, _, transform)) = card_query.get(entity) else {
        return false;
    };

    cursor_world.distance(rotation_handle_position(transform)) <= ROTATION_HANDLE_RADIUS
}

fn pick_card<F: bevy::ecs::query::QueryFilter>(
    cursor_world: Vec2,
    card_query: &Query<(Entity, &Card, &GlobalTransform, &Transform), F>,
) -> Option<(Entity, Transform)> {
    card_query
        .iter()
        .filter(|(_, card, global_transform, _)| {
            card.contains_point(global_transform, cursor_world)
        })
        .min_by(
            |(entity_a, _, _, transform_a), (entity_b, _, _, transform_b)| {
                transform_a
                    .translation
                    .z
                    .partial_cmp(&transform_b.translation.z)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .reverse()
                    .then_with(|| entity_a.index().cmp(&entity_b.index()))
            },
        )
        .map(|(entity, _, _, transform)| (entity, *transform))
}

pub fn cursor_is_over_scene(window: &Window, cursor_position: Vec2) -> bool {
    cursor_position.x > SIDEBAR_WIDTH && cursor_position.y < window.height() - TOOLBAR_HEIGHT
}

fn cursor_is_over_sidebar(window: &Window, cursor_position: Vec2) -> bool {
    cursor_position.x <= SIDEBAR_WIDTH && cursor_position.y < window.height() - TOOLBAR_HEIGHT
}

fn cursor_world_position(
    camera_query: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    cursor_position: Vec2,
) -> Option<Vec2> {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return None;
    };

    camera
        .viewport_to_world_2d(camera_transform, cursor_position)
        .ok()
}

fn parse_ui_color(input: &str) -> Option<Color> {
    let input = input.trim().trim_start_matches('#');
    if input.is_empty() {
        return None;
    }

    Srgba::hex(input).ok().map(Color::Srgba)
}

fn spawn_editor_card(
    commands: &mut Commands,
    spawn_deps: &mut EditorCardSpawnDeps,
    card_param: &CardParam,
) -> Entity {
    let entity = spawn_card_by_card_param(
        commands,
        &spawn_deps.asset_server,
        &spawn_deps.config,
        spawn_deps.meshes.as_mut(),
        spawn_deps.materials.as_mut(),
        card_param,
        &spawn_deps.card_presets_config,
        &spawn_deps.card_specialized_registry,
    );
    commands.entity(entity).insert(EditorView);
    append_editor_card_overlays(commands, entity);
    entity
}

fn append_editor_card_overlays(_commands: &mut Commands, _entity: Entity) {}

#[derive(Clone, Copy)]
enum SceneFileFormat {
    Toml,
    Binary,
}

fn save_scene(
    card_query: &Query<(Entity, &Card, &Transform), Without<EditorDragPreview>>,
    format: SceneFileFormat,
) -> String {
    save_scene_to_path(card_query, &scene_path(format))
}

fn save_scene_to_path(
    card_query: &Query<(Entity, &Card, &Transform), Without<EditorDragPreview>>,
    path: &Path,
) -> String {
    let scene = EditorSceneFile {
        cards: card_query
            .iter()
            .map(|(_, card, transform)| card.to_card_param(transform))
            .collect(),
    };

    if let Some(parent) = path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return format!("导出失败：无法创建目录 {}: {error}", parent.display());
    }

    let format = match scene_file_format_from_path(path) {
        Ok(format) => format,
        Err(error) => return error,
    };

    let result = match format {
        SceneFileFormat::Toml => toml::to_string_pretty(&scene)
            .map_err(|error| error.to_string())
            .and_then(|raw| fs::write(path, raw).map_err(|error| error.to_string())),
        SceneFileFormat::Binary => {
            write_scene_binary(&scene, path).map_err(|error| error.to_string())
        }
    };

    match result {
        Ok(()) => format!("已导出 {} 张卡牌到 {}", scene.cards.len(), path.display()),
        Err(error) => format!("导出失败 {}: {error}", path.display()),
    }
}

fn load_scene(
    commands: &mut Commands,
    card_query: &Query<(Entity, &Card, &Transform), Without<EditorDragPreview>>,
    mut spawn_deps: EditorCardSpawnDeps,
    format: SceneFileFormat,
) -> String {
    load_scene_from_path(commands, card_query, &mut spawn_deps, &scene_path(format))
}

fn load_scene_from_path(
    commands: &mut Commands,
    card_query: &Query<(Entity, &Card, &Transform), Without<EditorDragPreview>>,
    spawn_deps: &mut EditorCardSpawnDeps,
    path: &Path,
) -> String {
    let format = match scene_file_format_from_path(path) {
        Ok(format) => format,
        Err(error) => return error,
    };

    let scene = match format {
        SceneFileFormat::Toml => match fs::read_to_string(&path) {
            Ok(raw) => toml::from_str::<EditorSceneFile>(&raw).map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        },
        SceneFileFormat::Binary => read_scene_binary(&path).map_err(|error| error.to_string()),
    };

    let scene = match scene {
        Ok(scene) => scene,
        Err(error) => {
            return format!("导入失败 {}: {error}", path.display());
        }
    };

    for (entity, _, _) in card_query.iter() {
        commands.entity(entity).despawn();
    }

    let cards = scene.cards;
    for card in &cards {
        spawn_editor_card(commands, spawn_deps, card);
    }

    format!("已从 {} 导入 {} 张卡牌", path.display(), cards.len())
}

fn editor_scene_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(EDITOR_SCENE_DIR)
}

fn scene_path(format: SceneFileFormat) -> PathBuf {
    let file_name = match format {
        SceneFileFormat::Toml => EDITOR_SCENE_TOML,
        SceneFileFormat::Binary => EDITOR_SCENE_BIN,
    };

    editor_scene_dir().join(file_name)
}

fn pick_scene_export_path() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("导出场景")
        .set_directory(editor_scene_dir())
        .set_file_name(EDITOR_SCENE_TOML)
        .add_filter("场景文件", &["toml", "bin"])
        .save_file()
}

fn pick_scene_import_path() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("导入场景")
        .set_directory(editor_scene_dir())
        .add_filter("场景文件", &["toml", "bin"])
        .pick_file()
}

fn scene_file_format_from_path(path: &Path) -> Result<SceneFileFormat, String> {
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return Err(format!(
            "文件格式不受支持 {}：请使用 .toml 或 .bin",
            path.display()
        ));
    };

    match extension.to_ascii_lowercase().as_str() {
        "toml" => Ok(SceneFileFormat::Toml),
        "bin" => Ok(SceneFileFormat::Binary),
        _ => Err(format!(
            "文件格式不受支持 {}：请使用 .toml 或 .bin",
            path.display()
        )),
    }
}

fn write_scene_binary(scene: &EditorSceneFile, path: &Path) -> std::io::Result<()> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"VDES");
    bytes.extend_from_slice(&(scene.cards.len() as u32).to_le_bytes());

    for card in &scene.cards {
        bytes.extend_from_slice(&card.prefab_id.to_le_bytes());
        bytes.extend_from_slice(&card.scene_param.position.x.to_le_bytes());
        bytes.extend_from_slice(&card.scene_param.position.y.to_le_bytes());
        bytes.extend_from_slice(&card.scene_param.rotation.to_le_bytes());
    }

    fs::write(path, bytes)
}

fn read_scene_binary(path: &Path) -> Result<EditorSceneFile, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() < 8 || &bytes[0..4] != b"VDES" {
        return Err("非法二进制场景文件头".into());
    }

    let mut cursor = 4usize;
    let count = read_u32(&bytes, &mut cursor)? as usize;

    if bytes.len() == 8 + count * 16 {
        let mut cards = Vec::with_capacity(count);
        for _ in 0..count {
            let prefab_id = read_u32(&bytes, &mut cursor)?;
            let position = Vec2::new(
                read_f32(&bytes, &mut cursor)?,
                read_f32(&bytes, &mut cursor)?,
            );
            let rotation = read_f32(&bytes, &mut cursor)?;
            cards.push(CardParam {
                scene_param: CardSceneParam { position, rotation },
                prefab_id,
            });
        }

        return Ok(EditorSceneFile { cards });
    }

    for _ in 0..count {
        let title_len = read_u16(&bytes, &mut cursor)? as usize;
        let title_range_end = cursor + title_len;
        if title_range_end > bytes.len() {
            return Err("场景文件截断：标题越界".into());
        }
        cursor = title_range_end;

        let mut translation = [0.0; 3];
        for value in &mut translation {
            *value = read_f32(&bytes, &mut cursor)?;
        }
        let mut size = [0.0; 2];
        for value in &mut size {
            *value = read_f32(&bytes, &mut cursor)?;
        }
    }

    Ok(EditorSceneFile { cards: Vec::new() })
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, String> {
    let end = *cursor + 2;
    if end > bytes.len() {
        return Err("场景文件截断：u16 越界".into());
    }
    let value = u16::from_le_bytes([bytes[*cursor], bytes[*cursor + 1]]);
    *cursor = end;
    Ok(value)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, String> {
    let end = *cursor + 4;
    if end > bytes.len() {
        return Err("场景文件截断：u32 越界".into());
    }
    let value = u32::from_le_bytes([
        bytes[*cursor],
        bytes[*cursor + 1],
        bytes[*cursor + 2],
        bytes[*cursor + 3],
    ]);
    *cursor = end;
    Ok(value)
}

fn read_f32(bytes: &[u8], cursor: &mut usize) -> Result<f32, String> {
    let end = *cursor + 4;
    if end > bytes.len() {
        return Err("场景文件截断：f32 越界".into());
    }
    let value = f32::from_le_bytes([
        bytes[*cursor],
        bytes[*cursor + 1],
        bytes[*cursor + 2],
        bytes[*cursor + 3],
    ]);
    *cursor = end;
    Ok(value)
}

pub mod editor_view {
    use bevy::camera::Camera2d;
    use bevy::prelude::{Commands, Component};

    #[derive(Component)]
    pub struct EditorView;

    pub(super) fn setup_editor_view(mut commands: Commands) {
        commands.spawn((Camera2d, EditorView));
    }
}
