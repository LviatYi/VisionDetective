use crate::AppView;
use crate::card::card_params::{
    CardParam, CardRuntimeSpecializedConfig, CardSceneParam, CardSpawnParams,
};
use crate::card::{CARD_SIZE, Card, spawn_card_by_card_param};
use crate::config::GameConfig;
use crate::config::card_config::CardPresetsConfig;
use crate::editor::editor_view::{EditorView, setup_editor_view};
use crate::game_view::main_view::cleanup_view;
use crate::physics::obstacle::Obstacle;
use crate::physics::vision::{build_vision_mesh, compute_visible_points};
use crate::scene::SceneLayer;
use bevy::camera::Projection;
use bevy::input::ButtonInput;
use bevy::picking::pointer::PointerButton;
use bevy::picking::prelude::{Drag, Move, Out, Over, Pointer, Press, Release, Scroll};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::{PrimaryWindow, Window};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f32::consts::{FRAC_PI_4, PI, TAU};
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
const EDITOR_STATE_TOML: &str = "editor-state.toml";
const CARD_ORDER_STEP: f32 = 1.0;
const DRAG_PREVIEW_ORDER: f32 = 20.0;
const ORDER_LABEL_FONT_SIZE: f32 = 13.0;
const EDITOR_ROTATION_STEP: f32 = PI / 6.0;
const EDITOR_SNAP_DISTANCE: f32 = 8.0;
const EDITOR_CAMERA_DEFAULT_ZOOM: f32 = 1.0;
const EDITOR_CAMERA_MIN_ZOOM: f32 = 0.35;
const EDITOR_CAMERA_MAX_ZOOM: f32 = 4.0;
const EDITOR_CAMERA_ZOOM_BASE: f32 = 1.12;

pub struct EditorPlugin;

pub trait CardEditorSpecialized {
    fn register_editor_systems(app: &mut App);
}

#[derive(Resource, Default)]
struct EditorPointerWorldPosition(Option<Vec2>);

#[derive(Resource, Default)]
pub struct EditorInteractionState {
    selected_entity: Option<Entity>,
    dragging_prefab: Option<u32>,
    drag_preview_entity: Option<Entity>,
    moving_entity: Option<MovingEntityState>,
    rotating_entity: Option<RotatingEntityState>,
    camera_panning: Option<EditorCameraPanState>,
    prefab_scroll_offset: f32,
    escape_consumed: bool,
    pending_exit_confirmation: bool,
    status_message: String,
}

impl EditorInteractionState {
    pub fn captures_pointer(&self) -> bool {
        self.dragging_prefab.is_some()
            || self.moving_entity.is_some()
            || self.rotating_entity.is_some()
            || self.camera_panning.is_some()
    }

    pub fn take_escape_consumed(&mut self) -> bool {
        let consumed = self.escape_consumed;
        self.escape_consumed = false;
        consumed
    }

    pub fn select_entity(&mut self, entity: Entity) {
        self.selected_entity = Some(entity);
    }

    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
    }

    pub fn request_exit_to_main_menu(&mut self, has_unsaved_changes: bool) -> bool {
        if !has_unsaved_changes {
            self.pending_exit_confirmation = false;
            return true;
        }

        if self.pending_exit_confirmation {
            self.pending_exit_confirmation = false;
            return true;
        }

        self.pending_exit_confirmation = true;
        self.status_message = "当前场景存在未保存修改。再次按 Esc 确认退出。".into();
        false
    }

    fn clear_exit_confirmation(&mut self) {
        self.pending_exit_confirmation = false;
    }
}

#[derive(Bundle)]
struct EditorButtonBundle {
    button: Button,
    node: Node,
    background_color: BackgroundColor,
    action: EditorButtonAction,
}

#[derive(Clone)]
pub struct MovingEntityState {
    entity: Entity,
    pointer_offset: Vec2,
    start_position: Vec2,
    axis_lock: EditorAxisLock,
    before_scene: EditorSceneFile,
    changed: bool,
}

#[derive(Clone)]
pub struct RotatingEntityState {
    entity: Entity,
    before_scene: EditorSceneFile,
    changed: bool,
}

#[derive(Clone, Copy)]
struct EditorCameraPanState {
    last_cursor_position: Vec2,
}

#[derive(Component)]
struct EditorStatusText;

#[derive(Component)]
struct EditorResetCameraButton;

#[derive(Component, Clone, Copy)]
enum EditorButtonAction {
    ExportScene,
    ImportScene,
    ResetCameraView,
}

#[derive(Component, Clone, Copy)]
struct PrefabPreviewButton {
    prefab_id: u32,
}

#[derive(Component)]
struct PrefabListContent;

#[derive(Component)]
struct EditorDragPreview;

#[derive(Component)]
struct EditorCardOrderText;

#[derive(Component)]
struct EditorVisionPreview;

#[derive(Component, Clone, Copy)]
pub struct EditorPlacedCard;

#[derive(Component, Clone)]
pub struct EditorSpecializedAuxiliaryCard;

#[derive(Component, Clone)]
pub struct EditorRuntimeSpecializedParam(pub CardRuntimeSpecializedConfig);

#[derive(Component, Clone)]
pub struct EditorLinkedEntities {
    pub entities: Vec<Entity>,
}

struct PrefabPreviewItem {
    id: u32,
    title: String,
    background_color: Color,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct EditorSceneFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<CardParam>,
}

#[derive(Serialize, Deserialize, Default)]
struct EditorPersistentState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_scene_path: Option<PathBuf>,
}

#[derive(Resource, Default)]
struct EditorFileState {
    current_scene_path: Option<PathBuf>,
}

#[derive(Resource, Default)]
pub struct EditorUndoHistory {
    undo_stack: Vec<EditorSceneFile>,
    redo_stack: Vec<EditorSceneFile>,
    dirty: bool,
}

impl EditorUndoHistory {
    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    fn mark_clean(&mut self) {
        self.dirty = false;
    }

    fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.dirty = false;
    }
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorInteractionState>()
            .init_resource::<EditorPointerWorldPosition>()
            .init_resource::<EditorUndoHistory>()
            .init_resource::<EditorFileState>()
            .add_systems(
                OnEnter(AppView::Editor),
                (
                    reset_editor_state,
                    reset_editor_history,
                    load_editor_file_state,
                    setup_editor_view,
                    setup_editor_vision_preview,
                    setup_editor_ui,
                    open_last_editor_scene,
                )
                    .chain(),
            )
            .add_systems(OnExit(AppView::Editor), cleanup_view::<EditorView>)
            .add_systems(
                Update,
                track_editor_pointer_world_position.run_if(in_state(AppView::Editor)),
            )
            .add_systems(
                Update,
                (
                    handle_toolbar_buttons,
                    handle_prefab_drag_start,
                    handle_prefab_list_scroll,
                    update_drag_preview_card,
                    cancel_prefab_drag_with_escape,
                    handle_prefab_drop,
                    handle_editor_camera_pan,
                    handle_editor_camera_zoom,
                    handle_scene_editing,
                    handle_card_order_wheel,
                    handle_card_order_shortcuts,
                    update_editor_card_order_text,
                    update_editor_camera_reset_button_visibility,
                    update_editor_vision_preview,
                    handle_editor_shortcuts,
                    update_editor_status_text,
                )
                    .after(track_editor_pointer_world_position)
                    .run_if(in_state(AppView::Editor)),
            )
            .add_systems(Update, draw_editor_gizmos.run_if(in_state(AppView::Editor)));
    }
}

fn reset_editor_state(mut state: ResMut<EditorInteractionState>) {
    *state = EditorInteractionState {
        status_message: "编辑器已就绪".into(),
        ..default()
    };
}

fn reset_editor_history(mut history: ResMut<EditorUndoHistory>) {
    history.clear();
}

fn load_editor_file_state(mut file_state: ResMut<EditorFileState>) {
    *file_state = read_editor_file_state();
}

fn open_last_editor_scene(
    mut commands: Commands,
    card_query: Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        Without<EditorDragPreview>,
    >,
    mut spawn_deps: CardSpawnParams<'_>,
    mut state: ResMut<EditorInteractionState>,
    mut history: ResMut<EditorUndoHistory>,
    file_state: Res<EditorFileState>,
) {
    let Some(path) = file_state.current_scene_path.as_deref() else {
        return;
    };
    if !path.exists() {
        state.status_message = format!("上次编辑文件不存在：{}", path.display());
        return;
    }

    state.status_message = load_scene_from_path(&mut commands, &card_query, &mut spawn_deps, path);
    if state.status_message.starts_with("已从") {
        history.clear();
        state.clear_exit_confirmation();
    }
}

fn setup_editor_vision_preview(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        Mesh2d(meshes.add(build_vision_mesh(&config, Vec2::ZERO, &[]))),
        MeshMaterial2d(materials.add(config.vision.fill_color())),
        Transform::from_translation(Vec3::new(
            0.0,
            0.0,
            SceneLayer::PlayerVision.get_layer_base_z(),
        )),
        Visibility::Hidden,
        EditorVisionPreview,
        EditorView,
    ));
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
            Pickable::IGNORE,
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
                .spawn(EditorButtonBundle {
                    button: Button,
                    node: Node {
                        position_type: PositionType::Absolute,
                        right: px(18.0),
                        bottom: px(18.0),
                        height: px(COMPACT_BUTTON_HEIGHT),
                        padding: UiRect::axes(px(12.0), px(6.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    background_color: BackgroundColor(Color::srgb(0.19, 0.29, 0.40)),
                    action: EditorButtonAction::ResetCameraView,
                })
                .insert((Pickable::default(), Visibility::Hidden, EditorResetCameraButton))
                .with_children(|button| {
                    button.spawn((
                        Text::new("回到原视角"),
                        TextFont {
                            font: ui_font.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Pickable::IGNORE,
                    ));
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
                            "操作说明\n1. 直接按住卡牌预览并拖到主场景，松开后会克隆一张。\n2. 左键拖动卡牌位置，坐标会吸附到整数。\n3. 右键拖动主场景可平移编辑器视角，空白处滚轮调整视角高度。\n4. 拖动右上角旋转控制点，角度按 30° 粒度吸附。\n5. 鼠标悬浮卡牌时滚轮调整其在相交卡牌组内的层级。\n6. PageUp / PageDown 调整层级，Home / End 置底或置顶。\n7. Delete 删除选中卡牌。\n8. Ctrl+Z 撤销，Ctrl+Shift+Z 重做。\n9. Ctrl+E 导出，Ctrl+I 导入。",
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
                    order: 0.0,
                },
                prefab_id: prefab.id,
                runtime_specialized_param: None,
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
        .insert(Pickable::default())
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: font.clone(),
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
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
            Pickable::default(),
            PrefabPreviewButton { prefab_id },
        ))
        .with_children(|button| {
            button
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|text_wrap| {
                    text_wrap.spawn((
                        Text::new(title.to_string()),
                        TextFont {
                            font: font.clone(),
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Pickable::IGNORE,
                    ));
                    text_wrap.spawn((
                        Text::new(format!("prefab_id: {prefab_id}")),
                        TextFont {
                            font: font.clone(),
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.72, 0.78, 0.86)),
                        Pickable::IGNORE,
                    ));
                });
        });
}

fn handle_toolbar_buttons(
    mut commands: Commands,
    mut press_events: MessageReader<Pointer<Press>>,
    action_query: Query<&EditorButtonAction>,
    card_query: Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        Without<EditorDragPreview>,
    >,
    mut camera_query: Query<
        (&mut Transform, &mut Projection),
        (With<Camera2d>, With<EditorView>, Without<Card>),
    >,
    mut spawn_deps: CardSpawnParams<'_>,
    mut state: ResMut<EditorInteractionState>,
    mut history: ResMut<EditorUndoHistory>,
    mut file_state: ResMut<EditorFileState>,
) {
    for event in press_events.read() {
        if event.button != PointerButton::Primary {
            continue;
        }
        let Ok(action) = action_query.get(event.entity) else {
            continue;
        };

        match action {
            EditorButtonAction::ExportScene => {
                state.status_message =
                    match pick_scene_export_path(file_state.current_scene_path.as_deref()) {
                        Some(path) => {
                            let message = save_scene_to_path(&card_query, &path);
                            if message.starts_with("已导出") {
                                history.mark_clean();
                                state.clear_exit_confirmation();
                                remember_editor_scene_path(&mut file_state, path);
                            }
                            message
                        }
                        None => "已取消导出".into(),
                    };
            }
            EditorButtonAction::ImportScene => {
                state.status_message =
                    match pick_scene_import_path(file_state.current_scene_path.as_deref()) {
                        Some(path) => {
                            let message = load_scene_from_path(
                                &mut commands,
                                &card_query,
                                &mut spawn_deps,
                                &path,
                            );
                            if message.starts_with("已从") {
                                history.clear();
                                state.clear_exit_confirmation();
                                remember_editor_scene_path(&mut file_state, path);
                            }
                            message
                        }
                        None => "已取消导入".into(),
                    };
                state.selected_entity = None;
            }
            EditorButtonAction::ResetCameraView => {
                let Ok((mut camera_transform, mut projection)) = camera_query.single_mut() else {
                    state.status_message = "回归视角失败：编辑器相机不存在".into();
                    continue;
                };
                camera_transform.translation.x = 0.0;
                camera_transform.translation.y = 0.0;
                camera_transform.scale = Vec3::ONE;
                if let Projection::Orthographic(orthographic) = &mut *projection {
                    orthographic.scale = EDITOR_CAMERA_DEFAULT_ZOOM;
                }
                state.camera_panning = None;
                state.status_message = "已回到原先视角".into();
            }
        }
    }
}

fn handle_prefab_drag_start(
    mut commands: Commands,
    mut press_events: MessageReader<Pointer<Press>>,
    preview_query: Query<&PrefabPreviewButton>,
    mut spawn_deps: CardSpawnParams<'_>,
    mut state: ResMut<EditorInteractionState>,
) {
    for event in press_events.read() {
        if event.button != PointerButton::Primary {
            continue;
        }
        let Ok(preview_button) = preview_query.get(event.entity) else {
            continue;
        };

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
                    order: 0.0,
                },
                prefab_id: preview_button.prefab_id,
                runtime_specialized_param: None,
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

fn track_editor_pointer_world_position(
    mut move_events: MessageReader<Pointer<Move>>,
    mut over_events: MessageReader<Pointer<Over>>,
    mut out_events: MessageReader<Pointer<Out>>,
    mut press_events: MessageReader<Pointer<Press>>,
    mut drag_events: MessageReader<Pointer<Drag>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut pointer_world: ResMut<EditorPointerWorldPosition>,
) {
    for event in move_events.read() {
        pointer_world.0 = cursor_world_position(&camera_query, event.pointer_location.position);
    }
    for event in over_events.read() {
        pointer_world.0 = event
            .hit
            .position
            .map(|position| position.truncate())
            .or_else(|| cursor_world_position(&camera_query, event.pointer_location.position));
    }
    for event in out_events.read() {
        pointer_world.0 = event
            .hit
            .position
            .map(|position| position.truncate())
            .or_else(|| cursor_world_position(&camera_query, event.pointer_location.position));
    }
    for event in press_events.read() {
        pointer_world.0 = event
            .hit
            .position
            .map(|position| position.truncate())
            .or_else(|| cursor_world_position(&camera_query, event.pointer_location.position));
    }
    for event in drag_events.read() {
        pointer_world.0 = cursor_world_position(&camera_query, event.pointer_location.position);
    }
}

fn handle_prefab_list_scroll(
    mut scroll_events: MessageReader<Pointer<Scroll>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    prefab_query: Query<&PrefabPreviewButton>,
    mut content_query: Query<&mut Node, With<PrefabListContent>>,
    mut state: ResMut<EditorInteractionState>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };

    let scroll_units: f32 = scroll_events
        .read()
        .filter(|event| cursor_is_over_sidebar(window, event.pointer_location.position))
        .map(|event| event.y)
        .sum();
    if scroll_units.abs() <= f32::EPSILON {
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
    pointer_world: Res<EditorPointerWorldPosition>,
    state: Res<EditorInteractionState>,
    mut preview_query: Query<(&mut Transform, &mut Visibility), With<EditorDragPreview>>,
) {
    let Some(preview_entity) = state.drag_preview_entity else {
        return;
    };

    let Some(world_position) = pointer_world.0 else {
        return;
    };
    let world_position = normalize_editor_position(world_position);

    let Ok((mut transform, mut visibility)) = preview_query.get_mut(preview_entity) else {
        return;
    };

    transform.translation.x = world_position.x;
    transform.translation.y = world_position.y;
    transform.translation.z = editor_global_z_from_order(DRAG_PREVIEW_ORDER);
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
    mut release_events: MessageReader<Pointer<Release>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    pointer_world: Res<EditorPointerWorldPosition>,
    mut preview_query: Query<&mut Transform, With<EditorDragPreview>>,
    card_query: Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        Without<EditorDragPreview>,
    >,
    mut state: ResMut<EditorInteractionState>,
    mut history: ResMut<EditorUndoHistory>,
) {
    let Some(prefab_id) = state.dragging_prefab else {
        return;
    };

    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(release_position) = release_events
        .read()
        .filter(|event| event.button == PointerButton::Primary)
        .map(|event| event.pointer_location.position)
        .last()
    else {
        return;
    };

    state.dragging_prefab = None;
    let preview_entity = state.drag_preview_entity.take();

    if !cursor_is_over_scene(window, release_position) {
        if let Some(entity) = preview_entity {
            commands.entity(entity).despawn();
        }
        state.status_message = "已取消放置：请在主场景区域松开鼠标".into();
        return;
    }

    let Some(world_position) = pointer_world.0 else {
        if let Some(entity) = preview_entity {
            commands.entity(entity).despawn();
        }
        state.status_message = "放置卡牌失败：无法映射到场景坐标".into();
        return;
    };
    let world_position = normalize_editor_position(world_position);

    let Some(entity) = preview_entity else {
        state.status_message = "放置卡牌失败：拖拽预览不存在".into();
        return;
    };

    let before_scene = collect_editor_scene_snapshot(&card_query);
    let placed_order =
        next_card_order_from_transforms(card_query.iter().map(|(_, _, transform, _, _)| transform));
    if let Ok(mut transform) = preview_query.get_mut(entity) {
        transform.translation.x = world_position.x;
        transform.translation.y = world_position.y;
        transform.translation.z = editor_global_z_from_order(placed_order);
        transform.rotation = Quat::IDENTITY;
    }

    commands
        .entity(entity)
        .remove::<EditorDragPreview>()
        .insert((Visibility::Visible, EditorPlacedCard));

    state.selected_entity = Some(entity);
    record_editor_undo(&mut history, before_scene);
    state.clear_exit_confirmation();
    state.status_message = format!(
        "已放置卡牌 #{} ({:.0}, {:.0})，层级 {:.0}",
        prefab_id, world_position.x, world_position.y, placed_order
    );
}

fn handle_editor_camera_pan(
    mut press_events: MessageReader<Pointer<Press>>,
    mut drag_events: MessageReader<Pointer<Drag>>,
    mut release_events: MessageReader<Pointer<Release>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut camera_query: Query<
        (&Camera, &GlobalTransform, &mut Transform),
        (With<Camera2d>, With<EditorView>),
    >,
    mut state: ResMut<EditorInteractionState>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };

    for event in press_events.read() {
        if state.dragging_prefab.is_some()
            || state.moving_entity.is_some()
            || state.rotating_entity.is_some()
        {
            continue;
        }
        if event.button != PointerButton::Secondary
            || !cursor_is_over_scene(window, event.pointer_location.position)
        {
            continue;
        }
        state.camera_panning = Some(EditorCameraPanState {
            last_cursor_position: event.pointer_location.position,
        });
        state.moving_entity = None;
        state.rotating_entity = None;
        state.clear_exit_confirmation();
        state.status_message = "正在拖动编辑器视角".into();
    }

    for event in drag_events.read() {
        if event.button != PointerButton::Secondary {
            continue;
        }
        let Some(camera_panning) = state.camera_panning.as_mut() else {
            continue;
        };

        let Ok((camera, camera_global_transform, mut camera_transform)) = camera_query.single_mut()
        else {
            continue;
        };
        let Ok(previous_world_position) = camera
            .viewport_to_world_2d(camera_global_transform, camera_panning.last_cursor_position)
        else {
            continue;
        };
        let Ok(current_world_position) =
            camera.viewport_to_world_2d(camera_global_transform, event.pointer_location.position)
        else {
            continue;
        };

        let pan_delta = previous_world_position - current_world_position;
        camera_transform.translation.x += pan_delta.x;
        camera_transform.translation.y += pan_delta.y;
        camera_panning.last_cursor_position = event.pointer_location.position;
    }

    let released = release_events
        .read()
        .any(|event| event.button == PointerButton::Secondary);
    if released && state.camera_panning.take().is_some() {
        state.status_message = "编辑器视角已更新".into();
    }
}

fn handle_editor_camera_zoom(
    mut scroll_events: MessageReader<Pointer<Scroll>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut camera_query: Query<
        (&Camera, &GlobalTransform, &mut Transform, &mut Projection),
        (With<Camera2d>, With<EditorView>),
    >,
    card_query: Query<(), (With<Card>, Without<EditorDragPreview>)>,
    reset_button_query: Query<(), With<EditorResetCameraButton>>,
    mut state: ResMut<EditorInteractionState>,
) {
    if state.captures_pointer() {
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };

    for event in scroll_events.read() {
        if event.y.abs() <= f32::EPSILON
            || !cursor_is_over_scene(window, event.pointer_location.position)
            || card_query.get(event.entity).is_ok()
            || reset_button_query.get(event.entity).is_ok()
        {
            continue;
        }

        let Ok((camera, camera_global_transform, mut camera_transform, mut projection)) =
            camera_query.single_mut()
        else {
            continue;
        };
        let Projection::Orthographic(orthographic) = &mut *projection else {
            continue;
        };
        let Ok(anchor_world_position) =
            camera.viewport_to_world_2d(camera_global_transform, event.pointer_location.position)
        else {
            continue;
        };

        let current_zoom = orthographic
            .scale
            .clamp(EDITOR_CAMERA_MIN_ZOOM, EDITOR_CAMERA_MAX_ZOOM);
        let next_zoom = (current_zoom * EDITOR_CAMERA_ZOOM_BASE.powf(-event.y))
            .clamp(EDITOR_CAMERA_MIN_ZOOM, EDITOR_CAMERA_MAX_ZOOM);
        if (next_zoom - orthographic.scale).abs() <= f32::EPSILON {
            continue;
        }

        let camera_position = camera_transform.translation.truncate();
        let zoom_ratio = next_zoom / current_zoom;
        let anchored_camera_position =
            anchor_world_position - (anchor_world_position - camera_position) * zoom_ratio;
        camera_transform.translation.x = anchored_camera_position.x;
        camera_transform.translation.y = anchored_camera_position.y;
        orthographic.scale = next_zoom;
        state.status_message = format!("编辑器视角高度已调整为 {:.2}x", next_zoom);
    }
}

fn handle_scene_editing(
    mut commands: Commands,
    mut press_events: MessageReader<Pointer<Press>>,
    mut drag_events: MessageReader<Pointer<Drag>>,
    mut release_events: MessageReader<Pointer<Release>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut card_queries: ParamSet<(
        Query<(Entity, &Card, &GlobalTransform, &Transform), Without<EditorDragPreview>>,
        Query<&mut Transform>,
        Query<
            (
                Entity,
                &Card,
                &Transform,
                Option<&EditorRuntimeSpecializedParam>,
                Option<&EditorSpecializedAuxiliaryCard>,
            ),
            Without<EditorDragPreview>,
        >,
    )>,
    linked_entities_query: Query<&EditorLinkedEntities>,
    mut state: ResMut<EditorInteractionState>,
    mut history: ResMut<EditorUndoHistory>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };

    for event in press_events.read() {
        if event.button != PointerButton::Primary
            || !cursor_is_over_scene(window, event.pointer_location.position)
        {
            continue;
        }
        let Some(cursor_world_position) =
            cursor_world_position(&camera_query, event.pointer_location.position)
        else {
            continue;
        };
        state.clear_exit_confirmation();
        let mut start_rotation = None;
        let mut start_move = None;
        let mut clear_selection = false;
        {
            let card_query = card_queries.p0();

            if let Some(selected_entity) = state.selected_entity
                && rotation_handle_contains_point(
                    selected_entity,
                    cursor_world_position,
                    &card_query,
                )
            {
                start_rotation = Some(selected_entity);
            } else if let Ok((entity, _, _, transform)) = card_query.get(event.entity) {
                start_move = Some((
                    entity,
                    transform.translation.truncate() - cursor_world_position,
                    transform.translation.truncate(),
                ));
            } else if !keyboard_input.pressed(KeyCode::ControlLeft)
                && !keyboard_input.pressed(KeyCode::ControlRight)
            {
                clear_selection = true;
            }
        }

        if let Some(entity) = start_rotation {
            let before_scene = {
                let snapshot_query = card_queries.p2();
                collect_editor_scene_snapshot(&snapshot_query)
            };
            state.rotating_entity = Some(RotatingEntityState {
                entity,
                before_scene,
                changed: false,
            });
            state.moving_entity = None;
            state.status_message = "正在旋转卡牌".into();
            return;
        }

        if let Some((entity, pointer_offset, start_position)) = start_move {
            let before_scene = {
                let snapshot_query = card_queries.p2();
                collect_editor_scene_snapshot(&snapshot_query)
            };
            state.selected_entity = Some(entity);
            state.rotating_entity = None;
            state.moving_entity = Some(MovingEntityState {
                entity,
                pointer_offset,
                start_position,
                axis_lock: EditorAxisLock::None,
                before_scene,
                changed: false,
            });
            state.status_message = format!("已选中卡牌 #{entity:?}，拖动中");
        } else if clear_selection {
            state.selected_entity = None;
            state.status_message = "已取消选中".into();
        }
    }

    for event in drag_events.read() {
        if event.button != PointerButton::Primary {
            continue;
        }
        let Some(cursor_world_position) =
            cursor_world_position(&camera_query, event.pointer_location.position)
        else {
            continue;
        };

        if let Some(moving) = state.moving_entity.as_mut() {
            let shift_pressed = keyboard_input.pressed(KeyCode::ShiftLeft)
                || keyboard_input.pressed(KeyCode::ShiftRight);
            let snap_cards = {
                let card_query = card_queries.p0();
                collect_editor_snap_cards(moving.entity, &card_query)
            };
            let raw_next = normalize_editor_position(cursor_world_position + moving.pointer_offset);
            let axis_lock = update_editor_axis_lock(
                &mut moving.axis_lock,
                raw_next,
                moving.start_position,
                shift_pressed,
            );
            let axis_locked_next =
                apply_editor_axis_lock(raw_next, moving.start_position, axis_lock);
            let next = snap_editor_card_position(axis_locked_next, &snap_cards, axis_lock);

            let mut transform_query = card_queries.p1();
            if let Ok(mut transform) = transform_query.get_mut(moving.entity) {
                moving.changed |=
                    transform.translation.x != next.x || transform.translation.y != next.y;
                transform.translation.x = next.x;
                transform.translation.y = next.y;
            }
        }

        let mut transform_query = card_queries.p1();
        if let Some(rotating) = state.rotating_entity.as_mut()
            && let Ok(mut transform) = transform_query.get_mut(rotating.entity)
        {
            let center = transform.translation.truncate();
            let angle =
                normalize_editor_rotation((cursor_world_position - center).to_angle() - FRAC_PI_4);
            let old_angle = transform.rotation.to_euler(EulerRot::XYZ).2;
            rotating.changed |= normalize_editor_rotation(old_angle - angle).abs() > f32::EPSILON;
            transform.rotation = Quat::from_rotation_z(angle);
        }
    }

    let released = release_events
        .read()
        .any(|event| event.button == PointerButton::Primary);
    if released {
        if let Some(moving) = state.moving_entity.take() {
            if moving.changed {
                record_editor_undo(&mut history, moving.before_scene);
                state.clear_exit_confirmation();
                state.status_message = "卡牌位置已更新".into();
            }
        } else if let Some(rotating) = state.rotating_entity.take() {
            if rotating.changed {
                record_editor_undo(&mut history, rotating.before_scene);
                state.clear_exit_confirmation();
                state.status_message = "卡牌旋转已更新".into();
            }
        }
    }

    if keyboard_input.just_pressed(KeyCode::Delete)
        && let Some(entity) = state.selected_entity.take()
    {
        let before_scene = {
            let snapshot_query = card_queries.p2();
            collect_editor_scene_snapshot(&snapshot_query)
        };
        despawn_editor_card_with_linked_entities(&mut commands, entity, &linked_entities_query);
        record_editor_undo(&mut history, before_scene);
        state.moving_entity = None;
        state.rotating_entity = None;
        state.clear_exit_confirmation();
        state.status_message = "已删除选中卡牌".into();
    }
}

fn despawn_editor_card_with_linked_entities(
    commands: &mut Commands,
    entity: Entity,
    linked_entities_query: &Query<&EditorLinkedEntities>,
) {
    if let Ok(linked_entities) = linked_entities_query.get(entity) {
        for linked_entity in &linked_entities.entities {
            commands.entity(*linked_entity).despawn();
        }
    }
    commands.entity(entity).despawn();
}

fn handle_editor_shortcuts(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    card_query: Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        Without<EditorDragPreview>,
    >,
    mut spawn_deps: CardSpawnParams<'_>,
    mut state: ResMut<EditorInteractionState>,
    mut history: ResMut<EditorUndoHistory>,
    mut file_state: ResMut<EditorFileState>,
) {
    let ctrl_pressed = keyboard_input.pressed(KeyCode::ControlLeft)
        || keyboard_input.pressed(KeyCode::ControlRight);

    if !ctrl_pressed {
        return;
    }

    if keyboard_input.just_pressed(KeyCode::KeyZ) {
        if keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight)
        {
            state.status_message =
                redo_editor_operation(&mut commands, &card_query, spawn_deps, &mut history);
        } else {
            state.status_message =
                undo_editor_operation(&mut commands, &card_query, spawn_deps, &mut history);
        }
        state.selected_entity = None;
        state.clear_exit_confirmation();
        return;
    }

    if keyboard_input.just_pressed(KeyCode::KeyE) {
        state.status_message =
            match pick_scene_export_path(file_state.current_scene_path.as_deref()) {
                Some(path) => {
                    let message = save_scene_to_path(&card_query, &path);
                    if message.starts_with("已导出") {
                        history.mark_clean();
                        state.clear_exit_confirmation();
                        remember_editor_scene_path(&mut file_state, path);
                    }
                    message
                }
                None => "已取消导出".into(),
            };
    }

    if keyboard_input.just_pressed(KeyCode::KeyS) {
        let Some(path) = file_state.current_scene_path.clone() else {
            state.status_message = "保存失败：尚未选择编辑文件，请先导出或导入场景".into();
            return;
        };
        state.status_message = save_scene_to_path(&card_query, &path);
        if state.status_message.starts_with("已导出") {
            history.mark_clean();
            state.clear_exit_confirmation();
            remember_editor_scene_path(&mut file_state, path);
        }
    }

    if keyboard_input.just_pressed(KeyCode::KeyI) {
        state.status_message =
            match pick_scene_import_path(file_state.current_scene_path.as_deref()) {
                Some(path) => {
                    let message =
                        load_scene_from_path(&mut commands, &card_query, &mut spawn_deps, &path);
                    if message.starts_with("已从") {
                        history.clear();
                        state.clear_exit_confirmation();
                        remember_editor_scene_path(&mut file_state, path);
                    }
                    message
                }
                None => "已取消导入".into(),
            };
        state.selected_entity = None;
    }
}

fn handle_card_order_shortcuts(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut card_queries: ParamSet<(
        Query<(Entity, &mut Transform), (With<Card>, Without<EditorDragPreview>)>,
        Query<
            (
                Entity,
                &Card,
                &Transform,
                Option<&EditorRuntimeSpecializedParam>,
                Option<&EditorSpecializedAuxiliaryCard>,
            ),
            Without<EditorDragPreview>,
        >,
    )>,
    mut state: ResMut<EditorInteractionState>,
    mut history: ResMut<EditorUndoHistory>,
) {
    if state.captures_pointer() {
        return;
    }

    let Some(selected_entity) = state.selected_entity else {
        return;
    };

    let order_delta = if keyboard_input.just_pressed(KeyCode::PageUp) {
        Some(CardOrderChange::Step(CARD_ORDER_STEP))
    } else if keyboard_input.just_pressed(KeyCode::PageDown) {
        Some(CardOrderChange::Step(-CARD_ORDER_STEP))
    } else if keyboard_input.just_pressed(KeyCode::End) {
        max_card_order_mut(&card_queries.p0())
            .map(|order| CardOrderChange::Set(order + CARD_ORDER_STEP))
    } else if keyboard_input.just_pressed(KeyCode::Home) {
        min_card_order_mut(&card_queries.p0())
            .map(|order| CardOrderChange::Set(order - CARD_ORDER_STEP))
    } else {
        None
    };

    let Some(order_delta) = order_delta else {
        return;
    };

    let before_scene = {
        let snapshot_query = card_queries.p1();
        collect_editor_scene_snapshot(&snapshot_query)
    };

    let mut card_query = card_queries.p0();
    let Ok((_, mut transform)) = card_query.get_mut(selected_entity) else {
        state.selected_entity = None;
        state.status_message = "调整层级失败：选中卡牌不存在".into();
        return;
    };

    let old_order = editor_local_order_from_transform(&transform);
    match order_delta {
        CardOrderChange::Step(delta) => {
            let next_order = editor_local_order_from_transform(&transform) + delta;
            transform.translation.z = editor_global_z_from_order(next_order);
        }
        CardOrderChange::Set(order) => transform.translation.z = editor_global_z_from_order(order),
    }
    let new_order = editor_local_order_from_transform(&transform);
    if (old_order - new_order).abs() <= f32::EPSILON {
        state.status_message = "卡牌层级未变化".into();
        return;
    }

    record_editor_undo(&mut history, before_scene);
    state.clear_exit_confirmation();
    state.status_message = format!("卡牌层级已更新为 {:.0}", new_order);
}

fn handle_card_order_wheel(
    mut scroll_events: MessageReader<Pointer<Scroll>>,
    mut card_queries: ParamSet<(
        Query<(Entity, &Card, &GlobalTransform, &Transform), Without<EditorDragPreview>>,
        Query<&mut Transform, (With<Card>, Without<EditorDragPreview>)>,
        Query<
            (
                Entity,
                &Card,
                &Transform,
                Option<&EditorRuntimeSpecializedParam>,
                Option<&EditorSpecializedAuxiliaryCard>,
            ),
            Without<EditorDragPreview>,
        >,
    )>,
    mut state: ResMut<EditorInteractionState>,
    mut history: ResMut<EditorUndoHistory>,
) {
    if state.captures_pointer() {
        return;
    }

    let mut hovered_scrolls = Vec::new();
    for event in scroll_events.read() {
        if event.y.abs() <= f32::EPSILON {
            continue;
        }
        hovered_scrolls.push((event.entity, event.y));
    }
    if hovered_scrolls.is_empty() {
        return;
    }

    let Some((hovered_entity, scroll_units)) =
        hovered_scrolls.into_iter().rev().find(|(entity, _)| {
            let card_query = card_queries.p0();
            card_query.get(*entity).is_ok()
        })
    else {
        return;
    };

    let cards = {
        let card_query = card_queries.p0();
        let cards = collect_card_order_snapshots(&card_query);
        cards
    };

    let group = connected_intersecting_card_group(hovered_entity, &cards);
    if group.len() <= 1 {
        state.status_message = "悬浮卡牌没有相交卡牌组，层级未变化".into();
        return;
    }

    let direction = if scroll_units > 0.0 {
        CardOrderDirection::Up
    } else {
        CardOrderDirection::Down
    };
    let step_count = scroll_units.abs().round().max(1.0) as usize;
    let assignments =
        shifted_card_order_assignments(&cards, &group, hovered_entity, direction, step_count);
    if assignments.is_empty() {
        state.status_message = "悬浮卡牌已在相交卡牌组边界，层级未变化".into();
        return;
    }

    let next_order = assignments
        .iter()
        .find(|(entity, _)| *entity == hovered_entity)
        .map(|(_, order)| *order)
        .unwrap_or_default();

    let before_scene = {
        let snapshot_query = card_queries.p2();
        collect_editor_scene_snapshot(&snapshot_query)
    };

    {
        let mut transform_query = card_queries.p1();
        for (entity, order) in assignments {
            if let Ok(mut transform) = transform_query.get_mut(entity) {
                transform.translation.z = editor_global_z_from_order(order);
            }
        }
    }

    state.selected_entity = Some(hovered_entity);
    record_editor_undo(&mut history, before_scene);
    state.clear_exit_confirmation();
    state.status_message = format!(
        "已通过滚轮调整相交卡牌组层级：卡牌 #{hovered_entity:?}，当前层级 {:.0}",
        next_order
    );
}

fn update_editor_card_order_text(
    card_query: Query<&Transform, With<Card>>,
    mut text_query: Query<(&ChildOf, &mut Text2d), With<EditorCardOrderText>>,
) {
    for (parent, mut text) in &mut text_query {
        let Ok(transform) = card_query.get(parent.parent()) else {
            continue;
        };
        **text = format!("order {:.0}", editor_local_order_from_transform(transform));
    }
}

fn update_editor_camera_reset_button_visibility(
    camera_query: Query<
        (&Transform, &Projection),
        (
            With<Camera2d>,
            With<EditorView>,
            Without<EditorResetCameraButton>,
        ),
    >,
    mut button_query: Query<&mut Visibility, With<EditorResetCameraButton>>,
) {
    let Ok((camera_transform, projection)) = camera_query.single() else {
        return;
    };
    let Ok(mut visibility) = button_query.single_mut() else {
        return;
    };

    let is_camera_offset = camera_transform.translation.truncate().length_squared() > f32::EPSILON;
    let is_camera_zoomed = match projection {
        Projection::Orthographic(orthographic) => {
            (orthographic.scale - EDITOR_CAMERA_DEFAULT_ZOOM).abs() > f32::EPSILON
        }
        _ => false,
    };
    *visibility = if is_camera_offset || is_camera_zoomed {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

fn update_editor_vision_preview(
    config: Res<GameConfig>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    pointer_world: Res<EditorPointerWorldPosition>,
    obstacle_query: Query<(&Transform, &Obstacle)>,
    mut preview_query: Query<(&Mesh2d, &mut Visibility), With<EditorVisionPreview>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Ok((preview_mesh, mut visibility)) = preview_query.single_mut() else {
        return;
    };

    let alt_pressed =
        keyboard_input.pressed(KeyCode::AltLeft) || keyboard_input.pressed(KeyCode::AltRight);
    let Some(center) = pointer_world.0 else {
        *visibility = Visibility::Hidden;
        return;
    };

    if !alt_pressed {
        *visibility = Visibility::Hidden;
        return;
    }

    let Some(mesh) = meshes.get_mut(&preview_mesh.0) else {
        *visibility = Visibility::Hidden;
        return;
    };

    let visible_points = compute_visible_points(&config, center, &obstacle_query);
    *mesh = build_vision_mesh(&config, center, &visible_points);
    *visibility = Visibility::Visible;
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

fn normalize_editor_position(position: Vec2) -> Vec2 {
    Vec2::new(position.x.round(), position.y.round())
}

fn normalize_editor_axis(axis: f32) -> f32 {
    axis.round()
}

fn normalize_editor_rotation(rotation: f32) -> f32 {
    let stepped = (rotation / EDITOR_ROTATION_STEP).round() * EDITOR_ROTATION_STEP;
    let normalized = (stepped + PI).rem_euclid(TAU) - PI;
    if normalized == -PI { PI } else { normalized }
}

fn normalize_editor_scene_param(scene_param: &CardSceneParam) -> CardSceneParam {
    CardSceneParam {
        position: normalize_editor_position(scene_param.position),
        rotation: normalize_editor_rotation(scene_param.rotation),
        order: normalize_editor_order(scene_param.order),
    }
}

fn normalize_editor_export_scene_param(scene_param: &CardSceneParam) -> CardSceneParam {
    println!("scene_param: {:?}", scene_param);
    let csp = CardSceneParam {
        position: Vec2::new(
            scene_param.position.x.round(),
            scene_param.position.y.round(),
        ),
        rotation: normalize_editor_rotation(scene_param.rotation),
        order: normalize_editor_order(scene_param.order),
    };

    println!("csp: {:?}", csp);
    csp
}

fn normalize_editor_order(order: f32) -> f32 {
    order.round().max(0.0)
}

fn editor_local_order_from_transform(transform: &Transform) -> f32 {
    normalize_editor_order(transform.translation.z - SceneLayer::Card.get_layer_base_z())
}

fn editor_global_z_from_order(order: f32) -> f32 {
    SceneLayer::Card.get_layer_base_z() + normalize_editor_order(order)
}

fn parse_ui_color(input: &str) -> Option<Color> {
    let input = input.trim().trim_start_matches('#');
    if input.is_empty() {
        return None;
    }

    Srgba::hex(input).ok().map(Color::Srgba)
}

enum CardOrderChange {
    Step(f32),
    Set(f32),
}

#[derive(Clone, Copy)]
enum CardOrderDirection {
    Up,
    Down,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditorAxisLock {
    None,
    Horizontal,
    Vertical,
}

#[derive(Clone)]
struct CardOrderSnapshot {
    entity: Entity,
    corners: [Vec2; 4],
    order: f32,
}

#[derive(Clone, Copy)]
struct EditorSnapCard {
    center: Vec2,
    half_size: Vec2,
}

impl EditorSnapCard {
    fn left(&self) -> f32 {
        self.center.x - self.half_size.x
    }

    fn right(&self) -> f32 {
        self.center.x + self.half_size.x
    }

    fn bottom(&self) -> f32 {
        self.center.y - self.half_size.y
    }

    fn top(&self) -> f32 {
        self.center.y + self.half_size.y
    }
}

fn update_editor_axis_lock(
    axis_lock: &mut EditorAxisLock,
    position: Vec2,
    start_position: Vec2,
    shift_pressed: bool,
) -> EditorAxisLock {
    if !shift_pressed {
        *axis_lock = EditorAxisLock::None;
        return EditorAxisLock::None;
    }

    if *axis_lock != EditorAxisLock::None {
        return *axis_lock;
    }

    let delta = position - start_position;
    *axis_lock = if delta.x.abs() >= delta.y.abs() {
        EditorAxisLock::Horizontal
    } else {
        EditorAxisLock::Vertical
    };
    *axis_lock
}

fn apply_editor_axis_lock(position: Vec2, start_position: Vec2, axis_lock: EditorAxisLock) -> Vec2 {
    match axis_lock {
        EditorAxisLock::None => position,
        EditorAxisLock::Horizontal => Vec2::new(position.x, start_position.y),
        EditorAxisLock::Vertical => Vec2::new(start_position.x, position.y),
    }
}

fn collect_editor_snap_cards<F: bevy::ecs::query::QueryFilter>(
    moving_entity: Entity,
    card_query: &Query<(Entity, &Card, &GlobalTransform, &Transform), F>,
) -> Vec<EditorSnapCard> {
    card_query
        .iter()
        .filter(|(entity, _, _, _)| *entity != moving_entity)
        .map(|(_, _, _, transform)| EditorSnapCard {
            center: transform.translation.truncate(),
            half_size: CARD_SIZE * 0.5,
        })
        .collect()
}

fn snap_editor_card_position(
    position: Vec2,
    snap_cards: &[EditorSnapCard],
    axis_lock: EditorAxisLock,
) -> Vec2 {
    let half_size = CARD_SIZE * 0.5;
    let x = if axis_lock != EditorAxisLock::Vertical {
        closest_editor_snap_axis(
            position.x,
            &[-half_size.x, 0.0, half_size.x],
            |card| [card.left(), card.center.x, card.right()],
            snap_cards,
        )
    } else {
        None
    };

    let y = if axis_lock != EditorAxisLock::Horizontal {
        closest_editor_snap_axis(
            position.y,
            &[-half_size.y, 0.0, half_size.y],
            |card| [card.bottom(), card.center.y, card.top()],
            snap_cards,
        )
    } else {
        None
    };

    Vec2::new(x.unwrap_or(position.x), y.unwrap_or(position.y))
}

fn closest_editor_snap_axis(
    position: f32,
    moving_offsets: &[f32],
    target_lines: impl Fn(&EditorSnapCard) -> [f32; 3],
    snap_cards: &[EditorSnapCard],
) -> Option<f32> {
    let mut best: Option<(f32, f32)> = None;

    for moving_offset in moving_offsets {
        for snap_card in snap_cards {
            for target_line in target_lines(snap_card) {
                let candidate_position = target_line - moving_offset;
                let distance = (candidate_position - position).abs();
                if distance > EDITOR_SNAP_DISTANCE {
                    continue;
                }
                if best
                    .map(|(best_distance, _)| distance < best_distance)
                    .unwrap_or(true)
                {
                    best = Some((distance, candidate_position));
                }
            }
        }
    }

    best.map(|(_, candidate)| normalize_editor_axis(candidate))
}

fn next_card_order_from_transforms<'a>(transforms: impl Iterator<Item = &'a Transform>) -> f32 {
    transforms
        .map(editor_local_order_from_transform)
        .reduce(f32::max)
        .unwrap_or(0.0)
        + CARD_ORDER_STEP
}

fn min_card_order_mut<F: bevy::ecs::query::QueryFilter>(
    query: &Query<(Entity, &mut Transform), F>,
) -> Option<f32> {
    query
        .iter()
        .map(|(_, transform)| editor_local_order_from_transform(transform))
        .reduce(f32::min)
}

fn max_card_order_mut<F: bevy::ecs::query::QueryFilter>(
    query: &Query<(Entity, &mut Transform), F>,
) -> Option<f32> {
    query
        .iter()
        .map(|(_, transform)| editor_local_order_from_transform(transform))
        .reduce(f32::max)
}

fn collect_card_order_snapshots<F: bevy::ecs::query::QueryFilter>(
    query: &Query<(Entity, &Card, &GlobalTransform, &Transform), F>,
) -> Vec<CardOrderSnapshot> {
    query
        .iter()
        .map(|(entity, _, _, transform)| CardOrderSnapshot {
            entity,
            corners: obstacle_card_corners(transform),
            order: editor_local_order_from_transform(transform),
        })
        .collect()
}

fn connected_intersecting_card_group(root: Entity, cards: &[CardOrderSnapshot]) -> Vec<Entity> {
    let mut group = Vec::new();
    let mut queue = VecDeque::from([root]);

    while let Some(entity) = queue.pop_front() {
        if group.contains(&entity) {
            continue;
        }
        group.push(entity);

        let Some(card) = cards.iter().find(|card| card.entity == entity) else {
            continue;
        };

        for other in cards {
            if group.contains(&other.entity) || queue.contains(&other.entity) {
                continue;
            }
            if oriented_rectangles_intersect(&card.corners, &other.corners) {
                queue.push_back(other.entity);
            }
        }
    }

    group
}

fn shifted_card_order_assignments(
    cards: &[CardOrderSnapshot],
    group: &[Entity],
    target: Entity,
    direction: CardOrderDirection,
    step_count: usize,
) -> Vec<(Entity, f32)> {
    let mut ordered_cards = cards
        .iter()
        .filter(|card| group.contains(&card.entity))
        .map(|card| (card.entity, card.order))
        .collect::<Vec<_>>();

    ordered_cards.sort_by(|(entity_a, order_a), (entity_b, order_b)| {
        order_a
            .partial_cmp(order_b)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| entity_a.index().cmp(&entity_b.index()))
    });

    let Some(current_index) = ordered_cards
        .iter()
        .position(|(entity, _)| *entity == target)
    else {
        return Vec::new();
    };

    let next_index = match direction {
        CardOrderDirection::Up => (current_index + step_count).min(ordered_cards.len() - 1),
        CardOrderDirection::Down => current_index.saturating_sub(step_count),
    };
    if next_index == current_index {
        return Vec::new();
    }

    let mut order_slots = ordered_cards
        .iter()
        .map(|(_, order)| *order)
        .collect::<Vec<_>>();
    if !order_slots.windows(2).all(|orders| orders[0] < orders[1]) {
        let base_order = order_slots.first().copied().unwrap_or_default();
        for (index, order) in order_slots.iter_mut().enumerate() {
            *order = base_order + index as f32 * CARD_ORDER_STEP;
        }
    }

    let moved_card = ordered_cards.remove(current_index);
    ordered_cards.insert(next_index, moved_card);

    ordered_cards
        .into_iter()
        .zip(order_slots)
        .map(|((entity, _), order)| (entity, order))
        .collect()
}

fn oriented_rectangles_intersect(a: &[Vec2; 4], b: &[Vec2; 4]) -> bool {
    rectangle_axes(a)
        .into_iter()
        .chain(rectangle_axes(b))
        .all(|axis| projections_overlap(project_polygon(a, axis), project_polygon(b, axis)))
}

fn rectangle_axes(corners: &[Vec2; 4]) -> [Vec2; 4] {
    [
        perpendicular_axis(corners[1] - corners[0]),
        perpendicular_axis(corners[2] - corners[1]),
        perpendicular_axis(corners[3] - corners[2]),
        perpendicular_axis(corners[0] - corners[3]),
    ]
}

fn perpendicular_axis(edge: Vec2) -> Vec2 {
    let axis = Vec2::new(-edge.y, edge.x);
    axis.try_normalize().unwrap_or(Vec2::X)
}

fn project_polygon(corners: &[Vec2; 4], axis: Vec2) -> (f32, f32) {
    let mut min = corners[0].dot(axis);
    let mut max = min;
    for corner in corners.iter().skip(1) {
        let projection = corner.dot(axis);
        min = min.min(projection);
        max = max.max(projection);
    }
    (min, max)
}

fn projections_overlap(a: (f32, f32), b: (f32, f32)) -> bool {
    a.0 <= b.1 && b.0 <= a.1
}

pub fn spawn_editor_card(
    commands: &mut Commands,
    spawn_deps: &mut CardSpawnParams<'_>,
    card_param: &CardParam,
) -> Entity {
    let normalized_card_param = CardParam {
        scene_param: normalize_editor_scene_param(&card_param.scene_param),
        prefab_id: card_param.prefab_id,
        runtime_specialized_param: card_param.runtime_specialized_param.clone(),
    };
    let entity = spawn_card_by_card_param(commands, spawn_deps, &normalized_card_param);
    commands.entity(entity).insert(EditorView);
    append_editor_card_overlays(
        commands,
        entity,
        &spawn_deps.asset_server,
        &spawn_deps.config,
    );
    entity
}

fn append_editor_card_overlays(
    commands: &mut Commands,
    entity: Entity,
    asset_server: &AssetServer,
    config: &GameConfig,
) {
    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            Text2d::new("order 0"),
            TextFont {
                font: asset_server.load(config.assets.default_font.clone()),
                font_size: ORDER_LABEL_FONT_SIZE,
                ..default()
            },
            TextColor(Color::srgb(0.88, 0.94, 1.0)),
            Anchor::BOTTOM_RIGHT,
            Transform::from_xyz(CARD_SIZE.x * 0.5 - 6.0, -CARD_SIZE.y * 0.5 + 6.0, 0.36),
            EditorCardOrderText,
            EditorView,
        ));
    });
}

fn collect_editor_scene_snapshot<F: bevy::ecs::query::QueryFilter>(
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        F,
    >,
) -> EditorSceneFile {
    let mut cards = card_query
        .iter()
        .filter_map(|(entity, card, transform, runtime, auxiliary)| {
            editor_card_to_scene_card(card, transform, runtime, auxiliary)
                .map(|card| (entity, card))
        })
        .collect::<Vec<_>>();

    cards.sort_by(|(entity_a, card_a), (entity_b, card_b)| {
        card_a
            .scene_param
            .order
            .partial_cmp(&card_b.scene_param.order)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| entity_a.index().cmp(&entity_b.index()))
    });

    EditorSceneFile {
        cards: cards.into_iter().map(|(_, card)| card).collect(),
    }
}

fn record_editor_undo(history: &mut EditorUndoHistory, before_scene: EditorSceneFile) {
    history.undo_stack.push(before_scene);
    history.redo_stack.clear();
    history.dirty = true;
}

fn undo_editor_operation(
    commands: &mut Commands,
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        Without<EditorDragPreview>,
    >,
    mut spawn_deps: CardSpawnParams<'_>,
    history: &mut EditorUndoHistory,
) -> String {
    let Some(previous_scene) = history.undo_stack.pop() else {
        return "没有可撤销的操作".into();
    };

    let current_scene = collect_editor_scene_snapshot(card_query);
    restore_editor_scene(commands, card_query, &mut spawn_deps, &previous_scene);
    history.redo_stack.push(current_scene);
    history.dirty = true;
    "已撤销上一步操作".into()
}

fn redo_editor_operation(
    commands: &mut Commands,
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        Without<EditorDragPreview>,
    >,
    mut spawn_deps: CardSpawnParams<'_>,
    history: &mut EditorUndoHistory,
) -> String {
    let Some(next_scene) = history.redo_stack.pop() else {
        return "没有可重做的操作".into();
    };

    let current_scene = collect_editor_scene_snapshot(card_query);
    restore_editor_scene(commands, card_query, &mut spawn_deps, &next_scene);
    history.undo_stack.push(current_scene);
    history.dirty = true;
    "已重做上一步操作".into()
}

fn restore_editor_scene(
    commands: &mut Commands,
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        Without<EditorDragPreview>,
    >,
    spawn_deps: &mut CardSpawnParams<'_>,
    scene: &EditorSceneFile,
) {
    for (entity, _, _, _, _) in card_query.iter() {
        commands.entity(entity).despawn();
    }

    for card in &scene.cards {
        let entity = spawn_editor_card(commands, spawn_deps, card);
        commands.entity(entity).insert(EditorPlacedCard);
    }
}

#[derive(Clone, Copy)]
enum SceneFileFormat {
    Toml,
    // Binary,
}

fn save_scene_to_path(
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        Without<EditorDragPreview>,
    >,
    path: &Path,
) -> String {
    let mut cards = card_query
        .iter()
        .filter_map(|(entity, card, transform, runtime, auxiliary)| {
            editor_card_to_scene_card(card, transform, runtime, auxiliary)
                .map(|card| (entity, card))
        })
        .collect::<Vec<_>>();
    cards.sort_by(|(entity_a, card_a), (entity_b, card_b)| {
        card_a
            .scene_param
            .order
            .partial_cmp(&card_b.scene_param.order)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| entity_a.index().cmp(&entity_b.index()))
    });

    let scene = EditorSceneFile {
        cards: cards
            .into_iter()
            .map(|(_, mut card)| {
                card.scene_param = normalize_editor_export_scene_param(&card.scene_param);
                card
            })
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
    };

    match result {
        Ok(()) => format!("已导出 {} 张卡牌到 {}", scene.cards.len(), path.display()),
        Err(error) => format!("导出失败 {}: {error}", path.display()),
    }
}

fn editor_card_to_scene_card(
    card: &Card,
    transform: &Transform,
    runtime: Option<&EditorRuntimeSpecializedParam>,
    auxiliary: Option<&EditorSpecializedAuxiliaryCard>,
) -> Option<CardParam> {
    if auxiliary.is_some() {
        return None;
    }

    let mut card_param = card.to_card_param(transform);
    card_param.scene_param = normalize_editor_scene_param(&CardSceneParam {
        position: transform.translation.truncate(),
        rotation: transform.rotation.to_euler(EulerRot::XYZ).2,
        order: editor_local_order_from_transform(transform),
    });
    card_param.runtime_specialized_param = runtime.map(|runtime| runtime.0.clone());
    Some(card_param)
}

fn load_scene_from_path(
    commands: &mut Commands,
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        Without<EditorDragPreview>,
    >,
    spawn_deps: &mut CardSpawnParams<'_>,
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
    };

    let scene = match scene {
        Ok(scene) => scene,
        Err(error) => {
            return format!("导入失败 {}: {error}", path.display());
        }
    };

    for (entity, _, _, _, _) in card_query.iter() {
        commands.entity(entity).despawn();
    }

    let cards = scene.cards;
    for card in &cards {
        let entity = spawn_editor_card(commands, spawn_deps, card);
        commands.entity(entity).insert(EditorPlacedCard);
    }

    format!("已从 {} 导入 {} 张卡牌", path.display(), cards.len())
}

fn editor_scene_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(EDITOR_SCENE_DIR)
}

fn editor_state_path() -> PathBuf {
    editor_scene_dir().join(EDITOR_STATE_TOML)
}

fn read_editor_file_state() -> EditorFileState {
    let path = editor_state_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return EditorFileState::default();
    };
    let Ok(state) = toml::from_str::<EditorPersistentState>(&raw) else {
        return EditorFileState::default();
    };

    EditorFileState {
        current_scene_path: state.last_scene_path,
    }
}

fn remember_editor_scene_path(file_state: &mut EditorFileState, path: PathBuf) {
    file_state.current_scene_path = Some(path);
    let persistent = EditorPersistentState {
        last_scene_path: file_state.current_scene_path.clone(),
    };
    let state_path = editor_state_path();
    if let Some(parent) = state_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(raw) = toml::to_string_pretty(&persistent) {
        let _ = fs::write(state_path, raw);
    }
}

fn pick_scene_export_path(current_path: Option<&Path>) -> Option<PathBuf> {
    let directory = current_path
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(editor_scene_dir);
    let file_name = current_path
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or(EDITOR_SCENE_TOML);

    FileDialog::new()
        .set_title("导出场景")
        .set_directory(directory)
        .set_file_name(file_name)
        .add_filter("场景文件", &["toml"])
        .save_file()
}

fn pick_scene_import_path(current_path: Option<&Path>) -> Option<PathBuf> {
    let directory = current_path
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(editor_scene_dir);

    FileDialog::new()
        .set_title("导入场景")
        .set_directory(directory)
        .add_filter("场景文件", &["toml"])
        .pick_file()
}

fn scene_file_format_from_path(path: &Path) -> Result<SceneFileFormat, String> {
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return Err(format!("文件格式不受支持 {}：请使用 .toml", path.display()));
    };

    match extension.to_ascii_lowercase().as_str() {
        "toml" => Ok(SceneFileFormat::Toml),
        _ => Err(format!("文件格式不受支持 {}：请使用 .toml", path.display())),
    }
}

pub mod editor_view {
    use crate::scene::get_layered_game_scene_camera2d_bundle;
    use bevy::prelude::{Commands, Component};

    #[derive(Component)]
    pub struct EditorView;

    pub(super) fn setup_editor_view(mut commands: Commands) {
        commands.spawn((get_layered_game_scene_camera2d_bundle(), EditorView));
    }
}
