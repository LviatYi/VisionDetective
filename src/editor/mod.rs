use crate::AppStatus;
use crate::asset::runtime_root;
use crate::card::card_params::{
    CardParam, CardRuntimeSpecializedConfig, CardSceneParam, SpawnCardSystemParams,
};
use crate::card::specialized::obstacle::Obstacle;
use crate::card::specialized::trap::Trap;
use crate::card::{Card, CardSpecializedRegistry, spawn_card_by_card_param};
use crate::coin::character::{
    CharacterCoin, CharacterCoinParam, character_coin_to_param, spawn_character_coin,
};
use crate::config::GameConfig;
use crate::config::card_config::CardPresetsConfig;
use crate::config::character_config::CharacterConfig;
use crate::editor::editor_view::{EditorView, setup_editor_view};
use crate::game_view::main_view::cleanup_view;
use crate::physics::vision::{build_vision_mesh, compute_visible_points};
use crate::scene::terrain::{
    TerrainBoundary, TerrainParam, TerrainSceneParam, TerrainType, random_terrain_seed,
};
use crate::scene::{SCENE_ROTATION_STEP, SceneLayer, SceneParam};
use crate::tools::Disable;
use bevy::asset::RenderAssetUsages;
use bevy::camera::Projection;
use bevy::input::ButtonInput;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::picking::pointer::PointerButton;
use bevy::picking::prelude::{Drag, Move, Out, Over, Pointer, Press, Release, Scroll};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::{PrimaryWindow, Window};
use geo::TriangulateEarcut;
use geo::{Coord as GeoCoord, LineString as GeoLineString, Polygon as GeoPolygon};
#[cfg(not(target_arch = "wasm32"))]
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
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
#[cfg(not(target_arch = "wasm32"))]
const EDITOR_SCENE_TOML: &str = "editor_scene.toml";
const EDITOR_STATE_TOML: &str = "editor-state.toml";
const CARD_ORDER_STEP: f32 = 1.0;
const DRAG_PREVIEW_ORDER: f32 = 20.0;
const ORDER_LABEL_FONT_SIZE: f32 = 13.0;
const EDITOR_ROTATION_STEP: f32 = SCENE_ROTATION_STEP;
const EDITOR_SNAP_DISTANCE: f32 = 8.0;
const EDITOR_TERRAIN_PICK_ALPHA: f32 = 0.01;
const EDITOR_CAMERA_DEFAULT_ZOOM: f32 = 1.0;
const EDITOR_CAMERA_MIN_ZOOM: f32 = 0.35;
const EDITOR_CAMERA_MAX_ZOOM: f32 = 4.0;
const EDITOR_CAMERA_ZOOM_BASE: f32 = 1.12;
const EDITOR_BOX_SELECT_MIN_SIZE: f32 = 4.0;
const TERRAIN_CLOSE_DISTANCE: f32 = 12.0;

pub struct EditorPlugin;

pub type CardEditorSystemInstaller = fn(&mut App);

pub struct CardEditorSystemRegistration {
    pub type_id: &'static str,
    pub system_installer: CardEditorSystemInstaller,
}

impl CardEditorSystemRegistration {
    pub const fn new(type_id: &'static str, system_installer: CardEditorSystemInstaller) -> Self {
        Self {
            type_id,
            system_installer,
        }
    }

    fn install(&self, app: &mut App) {
        (self.system_installer)(app);
    }
}

inventory::collect!(CardEditorSystemRegistration);

#[macro_export]
macro_rules! register_card_editor_systems {
    ($name:expr, $system_installer:path) => {
        inventory::submit! {
            $crate::editor::CardEditorSystemRegistration::new($name, $system_installer)
        }
    };
}

#[derive(Resource, Default)]
struct EditorPointerWorldPosition(Option<Vec2>);

#[derive(Resource, Default)]
pub struct EditorInteractionState {
    selected_entity: Option<Entity>,
    selected_entities: Vec<Entity>,
    dragging_prefab: Option<u32>,
    drag_preview_entity: Option<Entity>,
    moving_entity: Option<MovingEntityState>,
    rotating_entity: Option<RotatingEntityState>,
    camera_panning: Option<EditorCameraPanState>,
    box_selection: Option<EditorBoxSelectionState>,
    terrain_draw_mode: bool,
    terrain_drawing: Option<EditorTerrainDrawingState>,
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
            || self.box_selection.is_some()
            || self.terrain_drawing.is_some()
    }

    pub fn take_escape_consumed(&mut self) -> bool {
        let consumed = self.escape_consumed;
        self.escape_consumed = false;
        consumed
    }

    pub fn select_entity(&mut self, entity: Entity) {
        self.set_selection(vec![entity]);
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

    fn set_selection(&mut self, entities: Vec<Entity>) {
        self.selected_entity = entities.first().copied();
        self.selected_entities = entities;
    }

    fn clear_selection(&mut self) {
        self.selected_entity = None;
        self.selected_entities.clear();
    }

    fn selected_entities_for_move(&self, entity: Entity) -> Vec<Entity> {
        if self.selected_entities.contains(&entity) {
            let mut entities = vec![entity];
            entities.extend(
                self.selected_entities
                    .iter()
                    .copied()
                    .filter(|selected_entity| *selected_entity != entity),
            );
            entities
        } else {
            vec![entity]
        }
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
    pointer_offset: Vec2,
    start_position: Vec2,
    axis_lock: EditorAxisLock,
    moving_entities: Vec<MovingEntityMember>,
    before_scene: EditorSceneFile,
    changed: bool,
}

#[derive(Clone)]
struct MovingEntityMember {
    entity: Entity,
    start_position: Vec2,
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

#[derive(Clone, Copy)]
struct EditorBoxSelectionState {
    start_position: Vec2,
    current_position: Vec2,
}

#[derive(Clone)]
struct EditorTerrainDrawingState {
    points: Vec<Vec2>,
    current_position: Vec2,
    before_scene: EditorSceneFile,
}

#[derive(Component)]
struct EditorStatusText;

#[derive(Component)]
struct EditorResetCameraButton;

#[derive(Component, Clone, Copy)]
enum EditorButtonAction {
    ExportScene,
    ImportScene,
    DrawTrapTerrain,
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
struct EditorOrderText;

#[derive(Component)]
struct EditorTerrainOrderOverlayAttached;

#[derive(Component)]
struct EditorVisionPreview;

#[derive(Component, Clone, Copy)]
pub struct EditorPlacedCard;

#[derive(Component, Clone)]
struct EditorPlacedTerrain;

#[derive(Component, Clone)]
struct EditorTerrainData(TerrainParam);

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

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terrains: Vec<TerrainParam>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub character_coins: Vec<CharacterCoinParam>,
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
                OnEnter(AppStatus::Editor),
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
            .add_systems(OnExit(AppStatus::Editor), cleanup_view::<EditorView>)
            .add_systems(
                Update,
                track_editor_pointer_world_position.run_if(in_state(AppStatus::Editor)),
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
                    handle_terrain_drawing,
                    handle_editor_camera_pan,
                    handle_editor_camera_zoom,
                    handle_scene_editing,
                    handle_card_order_wheel,
                    handle_card_order_shortcuts,
                    append_editor_terrain_order_overlays,
                    update_editor_card_order_text,
                    update_editor_camera_reset_button_visibility,
                    update_editor_vision_preview,
                    handle_editor_shortcuts,
                    update_editor_status_text,
                )
                    .after(track_editor_pointer_world_position)
                    .run_if(in_state(AppStatus::Editor)),
            )
            .add_systems(
                Update,
                draw_editor_gizmos.run_if(in_state(AppStatus::Editor)),
            );

        for registration in inventory::iter::<CardEditorSystemRegistration> {
            registration.install(app);
        }
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
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    character_coin_query: Query<(Entity, &CharacterCoin, &Transform), With<EditorView>>,
    mut spawn_deps: SpawnCardSystemParams<'_>,
    character_config: Res<CharacterConfig>,
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

    state.status_message = load_scene_from_path(
        &mut commands,
        &card_query,
        &terrain_query,
        &character_coin_query,
        &mut spawn_deps,
        &character_config,
        path,
    );
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
    card_specialized_registry: Res<CardSpecializedRegistry>,
) {
    let ui_font = asset_server.load(config.assets.default_font.clone());
    let preview_items = prefab_preview_items(
        &card_presets_config,
        &config,
        card_specialized_registry.as_ref(),
    );

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
                    spawn_button(
                        toolbar,
                        &ui_font,
                        "绘制陷阱地形",
                        EditorButtonAction::DrawTrapTerrain,
                    );
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
                            "操作说明\n1. 直接按住卡牌预览并拖到主场景，松开后会克隆一张。\n2. 点击“绘制陷阱地形”后左键逐点绘制，点击起点附近或按 Enter 完成。\n3. 左键拖动卡牌位置，空白处左键拖动可框选多张卡牌。\n4. 拖动选中卡牌可整体移动，按住 Shift 仅横向或纵向移动。\n5. 右键拖动主场景可平移编辑器视角，空白处滚轮调整视角高度。\n6. 拖动右上角旋转控制点，角度按 30° 粒度吸附。\n7. 鼠标悬浮卡牌时滚轮调整其在相交卡牌组内的层级，PageUp/PageDown 调整选中对象层级。\n8. Delete 删除选中对象。\n9. Ctrl+Z 撤销，Ctrl+Shift+Z 重做。\n10. Ctrl+E 导出，Ctrl+I 导入。",
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
    card_specialized_registry: &CardSpecializedRegistry,
) -> Vec<PrefabPreviewItem> {
    card_presets_config
        .prefabs
        .iter()
        .map(|prefab| {
            let card_param = CardParam {
                scene_param: CardSceneParam {
                    instance_id: String::new(),
                    data: SceneParam {
                        position: Vec2::ZERO,
                        rotation: 0.0,
                        order: 0.0,
                        enable_if: None,
                        disable_if: None,
                        description: String::new(),
                    },
                },
                prefab_id: prefab.id,
                runtime_specialized_param: None,
            };
            let appearance = card_param.load_appearance(card_presets_config);
            let background_color = card_param.resolve_fill_color(
                config,
                card_presets_config,
                card_specialized_registry,
            );

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
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    character_coin_query: Query<(Entity, &CharacterCoin, &Transform), With<EditorView>>,
    mut camera_query: Query<
        (&mut Transform, &mut Projection),
        (
            With<Camera2d>,
            With<EditorView>,
            Without<Card>,
            Without<EditorPlacedTerrain>,
            Without<CharacterCoin>,
        ),
    >,
    mut spawn_deps: SpawnCardSystemParams<'_>,
    character_config: Res<CharacterConfig>,
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
                            let message = save_scene_to_path(
                                &card_query,
                                &terrain_query,
                                &character_coin_query,
                                &path,
                                &spawn_deps.card_presets_config,
                            );
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
                                &terrain_query,
                                &character_coin_query,
                                &mut spawn_deps,
                                &character_config,
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
                state.clear_selection();
            }
            EditorButtonAction::DrawTrapTerrain => {
                state.terrain_draw_mode = !state.terrain_draw_mode;
                state.terrain_drawing = None;
                state.moving_entity = None;
                state.rotating_entity = None;
                state.box_selection = None;
                state.clear_selection();
                state.status_message = if state.terrain_draw_mode {
                    "已进入陷阱地形绘制模式：左键逐点绘制，Enter 完成，Esc 取消".into()
                } else {
                    "已退出陷阱地形绘制模式".into()
                };
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
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    card_query: Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    mut spawn_deps: SpawnCardSystemParams<'_>,
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
            commands.entity(entity).try_despawn();
        }

        let start_position = cursor_world_position(&camera_query, event.pointer_location.position)
            .map(normalize_editor_position)
            .unwrap_or(Vec2::ZERO);
        let before_scene = collect_editor_scene_snapshot(&card_query, &terrain_query);
        let entity = spawn_editor_card(
            &mut commands,
            &mut spawn_deps,
            &CardParam {
                scene_param: CardSceneParam {
                    instance_id: String::new(),
                    data: SceneParam {
                        position: start_position,
                        rotation: 0.0,
                        order: 0.0,
                        enable_if: None,
                        disable_if: None,
                        description: String::new(),
                    },
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
        state.moving_entity = Some(MovingEntityState {
            pointer_offset: Vec2::ZERO,
            start_position,
            axis_lock: EditorAxisLock::None,
            moving_entities: vec![MovingEntityMember {
                entity,
                start_position,
            }],
            before_scene,
            changed: false,
        });
        state.rotating_entity = None;
        state.box_selection = None;
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
    if state.terrain_draw_mode {
        return;
    }

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

    let is_moving_preview = state
        .moving_entity
        .as_ref()
        .map(|moving| {
            moving
                .moving_entities
                .iter()
                .any(|moving_entity| moving_entity.entity == preview_entity)
        })
        .unwrap_or(false);
    if !is_moving_preview {
        transform.translation.x = world_position.x;
        transform.translation.y = world_position.y;
    }
    transform.translation.z = editor_global_z_from_order(DRAG_PREVIEW_ORDER);
    transform.rotation = Quat::IDENTITY;
    *visibility = Visibility::Visible;
}

pub(crate) fn cancel_prefab_drag_with_escape(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorInteractionState>,
) {
    if state.terrain_draw_mode && keyboard_input.just_pressed(KeyCode::Escape) {
        state.terrain_drawing = None;
        state.terrain_draw_mode = false;
        state.escape_consumed = true;
        state.status_message = "已取消陷阱地形绘制".into();
        return;
    }

    if state.dragging_prefab.is_none() || !keyboard_input.just_pressed(KeyCode::Escape) {
        return;
    }

    if let Some(entity) = state.drag_preview_entity.take() {
        commands.entity(entity).try_despawn();
    }
    state.dragging_prefab = None;
    state.moving_entity = None;
    state.escape_consumed = true;
    state.status_message = "已取消拖拽预制体".into();
}

fn handle_prefab_drop(
    mut commands: Commands,
    mut release_events: MessageReader<Pointer<Release>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut preview_query: Query<
        &mut Transform,
        (With<EditorDragPreview>, Without<EditorPlacedTerrain>),
    >,
    card_query: Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
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
            commands.entity(entity).try_despawn();
        }
        state.moving_entity = None;
        state.status_message = "已取消放置：请在主场景区域松开鼠标".into();
        return;
    }

    let Some(entity) = preview_entity else {
        state.moving_entity = None;
        state.status_message = "放置卡牌失败：拖拽预览不存在".into();
        return;
    };

    let before_scene = collect_editor_scene_snapshot(&card_query, &terrain_query);
    let placed_order =
        next_card_order_from_transforms(card_query.iter().map(|(_, _, transform, _, _)| transform));
    let world_position = if let Ok(mut transform) = preview_query.get_mut(entity) {
        transform.translation.z = editor_global_z_from_order(placed_order);
        transform.rotation = Quat::IDENTITY;
        transform.translation.truncate()
    } else {
        state.moving_entity = None;
        state.status_message = "放置卡牌失败：拖拽预览不存在".into();
        return;
    };

    commands
        .entity(entity)
        .remove::<EditorDragPreview>()
        .insert((Visibility::Visible, EditorPlacedCard));

    state.moving_entity = None;
    state.set_selection(vec![entity]);
    record_editor_undo(&mut history, before_scene);
    state.clear_exit_confirmation();
    state.status_message = format!(
        "已放置卡牌 #{} ({:.0}, {:.0})，层级 {:.0}",
        prefab_id, world_position.x, world_position.y, placed_order
    );
}

fn handle_terrain_drawing(
    mut commands: Commands,
    mut press_events: MessageReader<Pointer<Press>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    card_query: Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorRuntimeSpecializedParam>,
            Option<&EditorSpecializedAuxiliaryCard>,
        ),
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    mut spawn_deps: SpawnCardSystemParams<'_>,
    mut state: ResMut<EditorInteractionState>,
    mut history: ResMut<EditorUndoHistory>,
) {
    if !state.terrain_draw_mode {
        return;
    }

    if keyboard_input.just_pressed(KeyCode::Escape) {
        state.terrain_drawing = None;
        state.terrain_draw_mode = false;
        state.escape_consumed = true;
        state.status_message = "已取消陷阱地形绘制".into();
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };

    if let Some(cursor_position) = window.cursor_position()
        && let Some(world_position) = cursor_world_position(&camera_query, cursor_position)
        && let Some(drawing) = state.terrain_drawing.as_mut()
    {
        drawing.current_position = normalize_editor_position(world_position);
    }

    for event in press_events.read() {
        if event.button != PointerButton::Primary
            || !cursor_is_over_scene(window, event.pointer_location.position)
        {
            continue;
        }
        let Some(world_position) =
            cursor_world_position(&camera_query, event.pointer_location.position)
        else {
            continue;
        };
        let point = normalize_editor_position(world_position);

        if state.terrain_drawing.is_none() {
            state.terrain_drawing = Some(EditorTerrainDrawingState {
                points: Vec::new(),
                current_position: point,
                before_scene: collect_editor_scene_snapshot(&card_query, &terrain_query),
            });
        }

        let should_close = state
            .terrain_drawing
            .as_ref()
            .map(|drawing| {
                drawing.points.len() >= 3
                    && point.distance(drawing.points[0]) <= TERRAIN_CLOSE_DISTANCE
            })
            .unwrap_or(false);
        if should_close {
            finish_terrain_drawing(
                &mut commands,
                &card_query,
                &terrain_query,
                &mut spawn_deps,
                &mut state,
                &mut history,
            );
            return;
        }

        let Some(drawing) = state.terrain_drawing.as_mut() else {
            continue;
        };
        drawing.points.push(point);
        drawing.current_position = point;
        let point_count = drawing.points.len();
        state.clear_selection();
        state.moving_entity = None;
        state.rotating_entity = None;
        state.box_selection = None;
        state.status_message = format!(
            "正在绘制陷阱地形：已放置 {} 个点，Enter 完成，Esc 取消",
            point_count
        );
    }

    if keyboard_input.just_pressed(KeyCode::Enter) {
        finish_terrain_drawing(
            &mut commands,
            &card_query,
            &terrain_query,
            &mut spawn_deps,
            &mut state,
            &mut history,
        );
    }
}

fn finish_terrain_drawing<F: bevy::ecs::query::QueryFilter>(
    commands: &mut Commands,
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
    terrain_query: &Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    spawn_deps: &mut SpawnCardSystemParams<'_>,
    state: &mut EditorInteractionState,
    history: &mut EditorUndoHistory,
) {
    let Some(drawing) = state.terrain_drawing.take() else {
        return;
    };
    if drawing.points.len() < 3 {
        state.status_message = "陷阱地形至少需要 3 个点".into();
        return;
    }

    let order = next_scene_order(
        card_query.iter().map(|(_, _, transform, _, _)| transform),
        terrain_query.iter().map(|(_, transform, _, _)| transform),
    );
    let terrain = terrain_param_from_world_path(drawing.points, order);
    spawn_editor_terrain(
        commands,
        &terrain,
        spawn_deps.meshes.as_mut(),
        spawn_deps.materials.as_mut(),
    );
    record_editor_undo(history, drawing.before_scene);
    state.terrain_draw_mode = false;
    state.clear_exit_confirmation();
    state.status_message = "已添加陷阱地形".into();
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
            || state.box_selection.is_some()
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
        Query<
            (Entity, &Card, &GlobalTransform, &Transform),
            (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
        >,
        Query<&mut Transform, (With<Card>, Without<EditorPlacedTerrain>)>,
        Query<
            (
                Entity,
                &Card,
                &Transform,
                Option<&EditorRuntimeSpecializedParam>,
                Option<&EditorSpecializedAuxiliaryCard>,
            ),
            (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
        >,
    )>,
    linked_entities_query: Query<&EditorLinkedEntities>,
    terrain_query: Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    reset_button_query: Query<(), With<EditorResetCameraButton>>,
    mut state: ResMut<EditorInteractionState>,
    mut history: ResMut<EditorUndoHistory>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };

    for event in press_events.read() {
        if event.button != PointerButton::Primary
            || !cursor_is_over_scene(window, event.pointer_location.position)
            || reset_button_query.get(event.entity).is_ok()
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
        let mut start_terrain_selection = None;
        let mut start_box_selection = false;
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
            } else if let Some(entity) =
                terrain_entity_at_world_position(cursor_world_position, &terrain_query)
            {
                start_terrain_selection = Some(entity);
            } else if !keyboard_input.pressed(KeyCode::ControlLeft)
                && !keyboard_input.pressed(KeyCode::ControlRight)
            {
                start_box_selection = true;
            }
        }

        if let Some(entity) = start_rotation {
            let before_scene = {
                let snapshot_query = card_queries.p2();
                collect_editor_scene_snapshot(&snapshot_query, &terrain_query)
            };
            state.rotating_entity = Some(RotatingEntityState {
                entity,
                before_scene,
                changed: false,
            });
            state.moving_entity = None;
            state.box_selection = None;
            state.status_message = "正在旋转卡牌".into();
            return;
        }

        if let Some((entity, pointer_offset, start_position)) = start_move {
            let before_scene = {
                let snapshot_query = card_queries.p2();
                collect_editor_scene_snapshot(&snapshot_query, &terrain_query)
            };
            let selected_entities = state.selected_entities_for_move(entity);
            let moving_entities = {
                let card_query = card_queries.p0();
                selected_entities
                    .iter()
                    .filter_map(|entity| {
                        card_query.get(*entity).ok().map(|(_, _, _, transform)| {
                            MovingEntityMember {
                                entity: *entity,
                                start_position: transform.translation.truncate(),
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            };
            state.set_selection(selected_entities);
            state.rotating_entity = None;
            state.box_selection = None;
            state.moving_entity = Some(MovingEntityState {
                pointer_offset,
                start_position,
                axis_lock: EditorAxisLock::None,
                moving_entities,
                before_scene,
                changed: false,
            });
            state.status_message = format!("已选中卡牌 #{entity:?}，拖动中");
        } else if let Some(entity) = start_terrain_selection {
            state.set_selection(vec![entity]);
            state.rotating_entity = None;
            state.moving_entity = None;
            state.box_selection = None;
            state.status_message = format!("已选中地形 #{entity:?}");
        } else if start_box_selection {
            state.rotating_entity = None;
            state.moving_entity = None;
            state.box_selection = Some(EditorBoxSelectionState {
                start_position: cursor_world_position,
                current_position: cursor_world_position,
            });
            state.status_message = "正在框选卡牌".into();
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
                collect_editor_snap_cards(&moving.moving_entities, &card_query)
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
            let delta = next - moving.start_position;

            let mut transform_query = card_queries.p1();
            for moving_entity in &moving.moving_entities {
                if let Ok(mut transform) = transform_query.get_mut(moving_entity.entity) {
                    let next_position =
                        normalize_editor_position(moving_entity.start_position + delta);
                    moving.changed |= transform.translation.x != next_position.x
                        || transform.translation.y != next_position.y;
                    transform.translation.x = next_position.x;
                    transform.translation.y = next_position.y;
                }
            }
        }

        if let Some(box_selection) = state.box_selection.as_mut() {
            box_selection.current_position = cursor_world_position;
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
    if released && state.dragging_prefab.is_none() {
        if let Some(moving) = state.moving_entity.take() {
            if moving.changed {
                record_editor_undo(&mut history, moving.before_scene);
                state.clear_exit_confirmation();
                state.status_message = if moving.moving_entities.len() > 1 {
                    format!("已移动 {} 张卡牌", moving.moving_entities.len())
                } else {
                    "卡牌位置已更新".into()
                };
            }
        } else if let Some(rotating) = state.rotating_entity.take() {
            if rotating.changed {
                record_editor_undo(&mut history, rotating.before_scene);
                state.clear_exit_confirmation();
                state.status_message = "卡牌旋转已更新".into();
            }
        } else if let Some(box_selection) = state.box_selection.take() {
            let selected_entities = {
                let card_query = card_queries.p0();
                collect_box_selected_entities(box_selection, &card_query)
            };
            if selected_entities.is_empty() {
                state.clear_selection();
                state.status_message = "框选区域内没有卡牌".into();
            } else {
                let count = selected_entities.len();
                state.set_selection(selected_entities);
                state.status_message = format!("已框选 {count} 张卡牌");
            }
        }
    }

    if keyboard_input.just_pressed(KeyCode::Delete) && !state.selected_entities.is_empty() {
        let before_scene = {
            let snapshot_query = card_queries.p2();
            collect_editor_scene_snapshot(&snapshot_query, &terrain_query)
        };
        let selected_entities = std::mem::take(&mut state.selected_entities);
        let mut despawned_entities = HashSet::new();
        for entity in selected_entities {
            despawn_editor_card_with_linked_entities(
                &mut commands,
                entity,
                &linked_entities_query,
                &mut despawned_entities,
            );
        }
        state.clear_selection();
        record_editor_undo(&mut history, before_scene);
        state.moving_entity = None;
        state.rotating_entity = None;
        state.box_selection = None;
        state.clear_exit_confirmation();
        state.status_message = "已删除选中对象".into();
    }
}

fn despawn_editor_card_with_linked_entities(
    commands: &mut Commands,
    entity: Entity,
    linked_entities_query: &Query<&EditorLinkedEntities>,
    despawned_entities: &mut HashSet<Entity>,
) {
    if !despawned_entities.insert(entity) {
        return;
    }
    if let Ok(linked_entities) = linked_entities_query.get(entity) {
        for linked_entity in &linked_entities.entities {
            if despawned_entities.insert(*linked_entity) {
                commands.entity(*linked_entity).try_despawn();
            }
        }
    }
    commands.entity(entity).try_despawn();
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
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    character_coin_query: Query<(Entity, &CharacterCoin, &Transform), With<EditorView>>,
    mut spawn_deps: SpawnCardSystemParams<'_>,
    character_config: Res<CharacterConfig>,
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
            state.status_message = redo_editor_operation(
                &mut commands,
                &card_query,
                &terrain_query,
                spawn_deps,
                &mut history,
            );
        } else {
            state.status_message = undo_editor_operation(
                &mut commands,
                &card_query,
                &terrain_query,
                spawn_deps,
                &mut history,
            );
        }
        state.clear_selection();
        state.clear_exit_confirmation();
        return;
    }

    if keyboard_input.just_pressed(KeyCode::KeyE) {
        state.status_message =
            match pick_scene_export_path(file_state.current_scene_path.as_deref()) {
                Some(path) => {
                    let message = save_scene_to_path(
                        &card_query,
                        &terrain_query,
                        &character_coin_query,
                        &path,
                        &spawn_deps.card_presets_config,
                    );
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
        state.status_message = save_scene_to_path(
            &card_query,
            &terrain_query,
            &character_coin_query,
            &path,
            &spawn_deps.card_presets_config,
        );
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
                    let message = load_scene_from_path(
                        &mut commands,
                        &card_query,
                        &terrain_query,
                        &character_coin_query,
                        &mut spawn_deps,
                        &character_config,
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
        state.clear_selection();
    }
}

fn handle_card_order_shortcuts(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut card_queries: ParamSet<(
        Query<
            (Entity, &mut Transform),
            (
                With<Card>,
                Without<EditorPlacedTerrain>,
                Without<EditorDragPreview>,
            ),
        >,
        Query<
            (
                Entity,
                &Card,
                &Transform,
                Option<&EditorRuntimeSpecializedParam>,
                Option<&EditorSpecializedAuxiliaryCard>,
            ),
            (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
        >,
    )>,
    mut terrain_queries: ParamSet<(
        Query<(Entity, &mut Transform), (With<EditorPlacedTerrain>, Without<EditorDragPreview>)>,
        Query<(Entity, &Transform, &EditorTerrainData, Option<&Trap>), With<EditorPlacedTerrain>>,
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
        let max_card_order = max_card_order_mut(&card_queries.p0());
        let max_terrain_order = max_terrain_order_mut(&terrain_queries.p0());
        max_optional_order(max_card_order, max_terrain_order)
            .map(|order| CardOrderChange::Set(order + CARD_ORDER_STEP))
    } else if keyboard_input.just_pressed(KeyCode::Home) {
        let min_card_order = min_card_order_mut(&card_queries.p0());
        let min_terrain_order = min_terrain_order_mut(&terrain_queries.p0());
        min_optional_order(min_card_order, min_terrain_order)
            .map(|order| CardOrderChange::Set(order - CARD_ORDER_STEP))
    } else {
        None
    };

    let Some(order_delta) = order_delta else {
        return;
    };

    let before_scene = {
        let snapshot_query = card_queries.p1();
        let terrain_snapshot_query = terrain_queries.p1();
        collect_editor_scene_snapshot(&snapshot_query, &terrain_snapshot_query)
    };

    let mut card_query = card_queries.p0();
    if let Ok((_, mut transform)) = card_query.get_mut(selected_entity) {
        let old_order = editor_local_order_from_transform(&transform);
        apply_editor_order_change(&mut transform, order_delta);
        let new_order = editor_local_order_from_transform(&transform);
        if (old_order - new_order).abs() <= f32::EPSILON {
            state.status_message = "卡牌层级未变化".into();
            return;
        }

        record_editor_undo(&mut history, before_scene);
        state.clear_exit_confirmation();
        state.status_message = format!("卡牌层级已更新为 {:.0}", new_order);
        return;
    }
    drop(card_query);

    let mut terrain_query = terrain_queries.p0();
    let Ok((_, mut transform)) = terrain_query.get_mut(selected_entity) else {
        state.clear_selection();
        state.status_message = "调整层级失败：选中对象不存在".into();
        return;
    };

    let old_order = editor_local_order_from_transform(&transform);
    apply_editor_order_change(&mut transform, order_delta);
    let new_order = editor_local_order_from_transform(&transform);
    if (old_order - new_order).abs() <= f32::EPSILON {
        state.status_message = "地形层级未变化".into();
        return;
    }

    record_editor_undo(&mut history, before_scene);
    state.clear_exit_confirmation();
    state.status_message = format!("地形层级已更新为 {:.0}", new_order);
}

fn handle_card_order_wheel(
    mut scroll_events: MessageReader<Pointer<Scroll>>,
    mut card_queries: ParamSet<(
        Query<
            (Entity, &Card, &GlobalTransform, &Transform),
            (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
        >,
        Query<
            &mut Transform,
            (
                With<Card>,
                Without<EditorPlacedTerrain>,
                Without<EditorDragPreview>,
            ),
        >,
        Query<
            (
                Entity,
                &Card,
                &Transform,
                Option<&EditorRuntimeSpecializedParam>,
                Option<&EditorSpecializedAuxiliaryCard>,
            ),
            (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
        >,
    )>,
    terrain_query: Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
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
        collect_editor_scene_snapshot(&snapshot_query, &terrain_query)
    };

    {
        let mut transform_query = card_queries.p1();
        for (entity, order) in assignments {
            if let Ok(mut transform) = transform_query.get_mut(entity) {
                transform.translation.z = editor_global_z_from_order(order);
            }
        }
    }

    state.set_selection(vec![hovered_entity]);
    record_editor_undo(&mut history, before_scene);
    state.clear_exit_confirmation();
    state.status_message = format!(
        "已通过滚轮调整相交卡牌组层级：卡牌 #{hovered_entity:?}，当前层级 {:.0}",
        next_order
    );
}

fn update_editor_card_order_text(
    transform_query: Query<&Transform>,
    mut order_text_query: Query<(&ChildOf, &mut Text2d), With<EditorOrderText>>,
) {
    for (parent, mut text) in &mut order_text_query {
        let Ok(transform) = transform_query.get(parent.parent()) else {
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

    let selection_text = match state.selected_entities.as_slice() {
        [] => "当前选中: 无".into(),
        [entity] => format!("当前选中: #{entity:?}"),
        entities => format!(
            "当前选中: {} 个对象，主选中 #{:?}",
            entities.len(),
            entities[0]
        ),
    };

    **text = format!("{selection_text}\n{}", state.status_message);
}

fn draw_editor_gizmos(
    mut gizmos: Gizmos,
    state: Res<EditorInteractionState>,
    card_query: Query<(Entity, &Transform), (With<Card>, Without<EditorDragPreview>)>,
) {
    for selected_entity in &state.selected_entities {
        if let Ok((_, transform)) = card_query.get(*selected_entity) {
            draw_card_outline(&mut gizmos, transform, Color::srgb(0.32, 0.90, 0.95));
        }
    }

    if let Some(selected_entity) = state.selected_entity
        && let Ok((_, transform)) = card_query.get(selected_entity)
    {
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

    if let Some(box_selection) = state.box_selection {
        let (min, max) =
            editor_selection_bounds(box_selection.start_position, box_selection.current_position);
        let corners = [
            Vec2::new(min.x, min.y),
            Vec2::new(max.x, min.y),
            Vec2::new(max.x, max.y),
            Vec2::new(min.x, max.y),
        ];
        for index in 0..corners.len() {
            gizmos.line_2d(
                corners[index],
                corners[(index + 1) % corners.len()],
                Color::srgb(0.32, 0.90, 0.95),
            );
        }
    }

    if let Some(drawing) = state.terrain_drawing.as_ref() {
        let mut preview_path = drawing.points.clone();
        preview_path.push(drawing.current_position);
        draw_path_by_gizmo(&mut gizmos, &preview_path, Color::srgb(0.95, 0.24, 0.24));
    }
}

fn draw_card_outline(gizmos: &mut Gizmos, transform: &Transform, color: Color) {
    let corners = obstacle_card_corners(transform);
    for index in 0..corners.len() {
        gizmos.line_2d(corners[index], corners[(index + 1) % corners.len()], color);
    }
}

fn obstacle_card_corners(transform: &Transform) -> [Vec2; 4] {
    let half = Card::SIZE * 0.5;
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
    let half = Card::SIZE * 0.5;
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

fn editor_selection_bounds(a: Vec2, b: Vec2) -> (Vec2, Vec2) {
    (
        Vec2::new(a.x.min(b.x), a.y.min(b.y)),
        Vec2::new(a.x.max(b.x), a.y.max(b.y)),
    )
}

fn collect_box_selected_entities<F: bevy::ecs::query::QueryFilter>(
    box_selection: EditorBoxSelectionState,
    card_query: &Query<(Entity, &Card, &GlobalTransform, &Transform), F>,
) -> Vec<Entity> {
    let (min, max) =
        editor_selection_bounds(box_selection.start_position, box_selection.current_position);
    if (max.x - min.x).abs() < EDITOR_BOX_SELECT_MIN_SIZE
        && (max.y - min.y).abs() < EDITOR_BOX_SELECT_MIN_SIZE
    {
        return Vec::new();
    }

    card_query
        .iter()
        .filter_map(|(entity, _, _, transform)| {
            if selection_rect_intersects_card(min, max, transform) {
                Some(entity)
            } else {
                None
            }
        })
        .collect()
}

fn selection_rect_intersects_card(min: Vec2, max: Vec2, transform: &Transform) -> bool {
    let selection_corners = [
        Vec2::new(min.x, min.y),
        Vec2::new(max.x, min.y),
        Vec2::new(max.x, max.y),
        Vec2::new(min.x, max.y),
    ];
    oriented_rectangles_intersect(&selection_corners, &obstacle_card_corners(transform))
}

fn terrain_entity_at_world_position<F: bevy::ecs::query::QueryFilter>(
    world_position: Vec2,
    terrain_query: &Query<(Entity, &Transform, &EditorTerrainData, Option<&Trap>), F>,
) -> Option<Entity> {
    terrain_query
        .iter()
        .filter_map(|(entity, transform, terrain_data, trap_terrain)| {
            let contains = trap_terrain
                .map(|terrain| terrain.contains_world_point(transform, world_position))
                .unwrap_or_else(|| {
                    let local_position = transform
                        .to_matrix()
                        .inverse()
                        .transform_point3(world_position.extend(0.0))
                        .truncate();
                    path_contains_point(local_position, &terrain_data.0.path)
                });
            contains.then_some((entity, editor_local_order_from_transform(transform)))
        })
        .max_by(|(_, order_a), (_, order_b)| {
            order_a
                .partial_cmp(order_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, _)| entity)
}

fn path_contains_point(point: Vec2, path: &[Vec2]) -> bool {
    if path.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut previous = path.len() - 1;

    for current in 0..path.len() {
        let current_point = path[current];
        let previous_point = path[previous];
        let intersects = (current_point.y > point.y) != (previous_point.y > point.y)
            && point.x
                < (previous_point.x - current_point.x) * (point.y - current_point.y)
                    / (previous_point.y - current_point.y)
                    + current_point.x;
        if intersects {
            inside = !inside;
        }
        previous = current;
    }

    inside
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

fn normalize_editor_scene_param(param: &SceneParam) -> SceneParam {
    SceneParam {
        position: normalize_editor_position(param.position),
        rotation: normalize_editor_rotation(param.rotation),
        order: normalize_editor_order(param.order),
        enable_if: param.enable_if.clone(),
        disable_if: param.disable_if.clone(),
        description: param.description.clone(),
    }
}

fn normalize_editor_card_scene_param(param: &CardSceneParam) -> CardSceneParam {
    CardSceneParam {
        instance_id: param.instance_id.clone(),
        data: normalize_editor_scene_param(&param.data),
    }
}

fn normalize_editor_terrain_scene_param(param: &TerrainSceneParam) -> TerrainSceneParam {
    TerrainSceneParam(normalize_editor_scene_param(&param.0))
}

fn normalize_editor_order(order: f32) -> f32 {
    order.round().max(0.0)
}

fn editor_local_order_from_transform(transform: &Transform) -> f32 {
    normalize_editor_order(transform.translation.z - SceneLayer::SceneObjects.get_layer_base_z())
}

fn editor_global_z_from_order(order: f32) -> f32 {
    SceneLayer::SceneObjects.get_layer_base_z() + normalize_editor_order(order)
}

pub fn draw_path_by_gizmo(gizmos: &mut Gizmos, world_path: &[Vec2], color: Color) {
    if world_path.len() < 2 {
        return;
    }

    for index in 0..world_path.len() {
        let a = world_path[index];
        let b = world_path[(index + 1) % world_path.len()];
        gizmos.line_2d(a, b, color);
        gizmos.circle_2d(a, 3.0, color);
    }
}

#[derive(Clone, Copy)]
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
    moving_entities: &[MovingEntityMember],
    card_query: &Query<(Entity, &Card, &GlobalTransform, &Transform), F>,
) -> Vec<EditorSnapCard> {
    card_query
        .iter()
        .filter(|(entity, _, _, _)| {
            !moving_entities
                .iter()
                .any(|moving_entity| moving_entity.entity == *entity)
        })
        .map(|(_, _, _, transform)| EditorSnapCard {
            center: transform.translation.truncate(),
            half_size: Card::SIZE * 0.5,
        })
        .collect()
}

fn snap_editor_card_position(
    position: Vec2,
    snap_cards: &[EditorSnapCard],
    axis_lock: EditorAxisLock,
) -> Vec2 {
    let half_size = Card::SIZE * 0.5;
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

fn next_scene_order<'a>(
    card_transforms: impl Iterator<Item = &'a Transform>,
    terrain_transforms: impl Iterator<Item = &'a Transform>,
) -> f32 {
    card_transforms
        .chain(terrain_transforms)
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

fn min_terrain_order_mut<F: bevy::ecs::query::QueryFilter>(
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

fn max_terrain_order_mut<F: bevy::ecs::query::QueryFilter>(
    query: &Query<(Entity, &mut Transform), F>,
) -> Option<f32> {
    query
        .iter()
        .map(|(_, transform)| editor_local_order_from_transform(transform))
        .reduce(f32::max)
}

fn min_optional_order(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(order), None) | (None, Some(order)) => Some(order),
        (None, None) => None,
    }
}

fn max_optional_order(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(order), None) | (None, Some(order)) => Some(order),
        (None, None) => None,
    }
}

fn apply_editor_order_change(transform: &mut Transform, order_delta: CardOrderChange) {
    match order_delta {
        CardOrderChange::Step(delta) => {
            let next_order = editor_local_order_from_transform(transform) + delta;
            transform.translation.z = editor_global_z_from_order(next_order);
        }
        CardOrderChange::Set(order) => transform.translation.z = editor_global_z_from_order(order),
    }
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
    spawn_deps: &mut SpawnCardSystemParams<'_>,
    card_param: &CardParam,
) -> Entity {
    let normalized_card_param = CardParam {
        scene_param: normalize_editor_card_scene_param(&card_param.scene_param),
        prefab_id: card_param.prefab_id,
        runtime_specialized_param: card_param.runtime_specialized_param.clone(),
    };
    let entity = spawn_card_by_card_param(commands, spawn_deps, &normalized_card_param, true);
    commands
        .entity(entity)
        .remove::<Disable>()
        .insert((EditorView, Visibility::Visible));
    append_editor_card_overlays(
        commands,
        entity,
        &spawn_deps.asset_server,
        &spawn_deps.config,
    );
    entity
}

fn spawn_editor_terrain(
    commands: &mut Commands,
    terrain: &TerrainParam,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> Entity {
    let normalized_scene_param = normalize_editor_terrain_scene_param(&terrain.scene_param);
    let mut entity = commands.spawn((
        Transform::from_translation(
            normalized_scene_param
                .position
                .extend(editor_global_z_from_order(normalized_scene_param.order)),
        )
        .with_rotation(Quat::from_rotation_z(normalized_scene_param.rotation)),
        EditorTerrainData(terrain.clone()),
        TerrainBoundary::new(terrain.path.clone(), terrain.terrain_type),
        EditorPlacedTerrain,
        EditorView,
    ));

    if let Some(mesh) = editor_terrain_pick_mesh(&terrain.path) {
        entity.insert((
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(materials.add(Color::srgba(1.0, 1.0, 1.0, EDITOR_TERRAIN_PICK_ALPHA))),
            Pickable::default(),
        ));
    }

    if terrain.terrain_type == TerrainType::Trap {
        entity.insert(Trap::new(terrain.path.clone()));
    }

    entity.id()
}

fn editor_terrain_pick_mesh(local_path: &[Vec2]) -> Option<Mesh> {
    if local_path.len() < 3 {
        return None;
    }

    let mut coordinates = local_path
        .iter()
        .map(|point| GeoCoord {
            x: point.x,
            y: point.y,
        })
        .collect::<Vec<_>>();
    coordinates.push(GeoCoord {
        x: local_path[0].x,
        y: local_path[0].y,
    });
    let polygon = GeoPolygon::new(GeoLineString::from(coordinates), vec![]);

    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for triangle in polygon.earcut_triangles() {
        let base = positions.len() as u32;
        for point in [triangle.v1(), triangle.v2(), triangle.v3()] {
            positions.push([point.x, point.y, 0.0]);
            uvs.push([0.0, 0.0]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    if positions.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

fn terrain_param_from_world_path(world_path: Vec<Vec2>, order: f32) -> TerrainParam {
    let (min, max) = path_bounds(&world_path).unwrap_or((Vec2::ZERO, Vec2::ZERO));
    let center = normalize_editor_position((min + max) * 0.5);
    let local_path = world_path
        .into_iter()
        .map(|point| normalize_editor_position(point - center))
        .collect();

    TerrainParam {
        path: local_path,
        seed: random_terrain_seed(),
        scene_param: TerrainSceneParam(SceneParam {
            position: center,
            order,
            ..default()
        }),
        ..default()
    }
}

fn editor_terrain_to_scene_terrain(
    transform: &Transform,
    terrain_data: &EditorTerrainData,
    trap_terrain: Option<&Trap>,
) -> TerrainParam {
    let mut terrain = terrain_data.0.clone();
    if let Some(trap_terrain) = trap_terrain {
        terrain.path = trap_terrain.local_path.clone();
    }
    terrain.scene_param = normalize_editor_terrain_scene_param(&TerrainSceneParam(SceneParam {
        position: transform.translation.truncate(),
        rotation: transform.rotation.to_euler(EulerRot::XYZ).2,
        order: editor_local_order_from_transform(transform),
        enable_if: terrain.scene_param.enable_if.clone(),
        disable_if: terrain.scene_param.disable_if.clone(),
        description: terrain.scene_param.description.clone(),
    }));
    terrain
}

fn path_bounds(path: &[Vec2]) -> Option<(Vec2, Vec2)> {
    let mut points = path.iter();
    let first = *points.next()?;
    let mut min = first;
    let mut max = first;

    for &point in points {
        min = min.min(point);
        max = max.max(point);
    }

    Some((min, max))
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
            Transform::from_xyz(Card::SIZE.x * 0.5 - 6.0, -Card::SIZE.y * 0.5 + 6.0, 0.36),
            EditorOrderText,
            EditorView,
        ));
    });
}

fn append_editor_terrain_order_overlays(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config: Res<GameConfig>,
    terrain_query: Query<
        (Entity, &EditorTerrainData),
        (
            With<EditorPlacedTerrain>,
            Without<EditorTerrainOrderOverlayAttached>,
        ),
    >,
) {
    for (entity, terrain) in &terrain_query {
        let (_, max) = path_bounds(&terrain.0.path).unwrap_or((Vec2::ZERO, Vec2::ZERO));
        commands
            .entity(entity)
            .insert(EditorTerrainOrderOverlayAttached)
            .with_children(|parent| {
                parent.spawn((
                    Text2d::new("order 0"),
                    TextFont {
                        font: asset_server.load(config.assets.default_font.clone()),
                        font_size: ORDER_LABEL_FONT_SIZE,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.94, 1.0)),
                    Anchor::BOTTOM_RIGHT,
                    Transform::from_xyz(max.x, max.y, 0.36),
                    EditorOrderText,
                    EditorView,
                ));
            });
    }
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
    terrain_query: &Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
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
            .data
            .order
            .partial_cmp(&card_b.scene_param.data.order)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| entity_a.index().cmp(&entity_b.index()))
    });

    let mut terrains = terrain_query
        .iter()
        .map(|(entity, transform, terrain_data, trap_terrain)| {
            (
                entity,
                editor_terrain_to_scene_terrain(transform, terrain_data, trap_terrain),
            )
        })
        .collect::<Vec<_>>();
    terrains.sort_by(|(entity_a, terrain_a), (entity_b, terrain_b)| {
        terrain_a
            .scene_param
            .order
            .partial_cmp(&terrain_b.scene_param.order)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| entity_a.index().cmp(&entity_b.index()))
    });

    EditorSceneFile {
        cards: cards.into_iter().map(|(_, card)| card).collect(),
        terrains: terrains.into_iter().map(|(_, terrain)| terrain).collect(),
        character_coins: Vec::new(),
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
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: &Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    mut spawn_deps: SpawnCardSystemParams<'_>,
    history: &mut EditorUndoHistory,
) -> String {
    let Some(previous_scene) = history.undo_stack.pop() else {
        return "没有可撤销的操作".into();
    };

    let current_scene = collect_editor_scene_snapshot(card_query, terrain_query);
    restore_editor_scene(
        commands,
        card_query,
        terrain_query,
        &mut spawn_deps,
        &previous_scene,
    );
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
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: &Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    mut spawn_deps: SpawnCardSystemParams<'_>,
    history: &mut EditorUndoHistory,
) -> String {
    let Some(next_scene) = history.redo_stack.pop() else {
        return "没有可重做的操作".into();
    };

    let current_scene = collect_editor_scene_snapshot(card_query, terrain_query);
    restore_editor_scene(
        commands,
        card_query,
        terrain_query,
        &mut spawn_deps,
        &next_scene,
    );
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
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: &Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    spawn_deps: &mut SpawnCardSystemParams<'_>,
    scene: &EditorSceneFile,
) {
    for (entity, _, _, _, _) in card_query.iter() {
        commands.entity(entity).try_despawn();
    }
    for (entity, _, _, _) in terrain_query.iter() {
        commands.entity(entity).try_despawn();
    }

    for card in &scene.cards {
        let entity = spawn_editor_card(commands, spawn_deps, card);
        commands.entity(entity).insert(EditorPlacedCard);
    }
    for terrain in &scene.terrains {
        spawn_editor_terrain(
            commands,
            terrain,
            spawn_deps.meshes.as_mut(),
            spawn_deps.materials.as_mut(),
        );
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
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: &Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    character_coin_query: &Query<(Entity, &CharacterCoin, &Transform), With<EditorView>>,
    path: &Path,
    card_presets_config: &CardPresetsConfig,
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
            .data
            .order
            .partial_cmp(&card_b.scene_param.data.order)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| entity_a.index().cmp(&entity_b.index()))
    });

    let scene = EditorSceneFile {
        cards: cards
            .into_iter()
            .map(|(_, mut card)| {
                card.scene_param = normalize_editor_card_scene_param(&card.scene_param);
                card.scene_param.data.description = make_description(&card, card_presets_config);
                card
            })
            .collect(),
        terrains: terrain_query
            .iter()
            .map(|(_, transform, terrain_data, trap_terrain)| {
                let mut terrain =
                    editor_terrain_to_scene_terrain(transform, terrain_data, trap_terrain);
                terrain.scene_param = normalize_editor_terrain_scene_param(&terrain.scene_param);
                terrain
            })
            .collect(),
        character_coins: collect_editor_character_coins(character_coin_query),
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
        Ok(()) => format!(
            "已导出 {} 张卡牌、{} 个地形、{} 个角色硬币到 {}",
            scene.cards.len(),
            scene.terrains.len(),
            scene.character_coins.len(),
            path.display()
        ),
        Err(error) => format!("导出失败 {}: {error}", path.display()),
    }
}

fn collect_editor_character_coins(
    character_coin_query: &Query<(Entity, &CharacterCoin, &Transform), With<EditorView>>,
) -> Vec<CharacterCoinParam> {
    let mut character_coins = character_coin_query
        .iter()
        .map(|(entity, coin, transform)| (entity, character_coin_to_param(coin, transform)))
        .collect::<Vec<_>>();

    character_coins.sort_by(|(entity_a, coin_a), (entity_b, coin_b)| {
        coin_a
            .scene_param
            .order
            .partial_cmp(&coin_b.scene_param.order)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| entity_a.index().cmp(&entity_b.index()))
    });

    character_coins
        .into_iter()
        .map(|(_, mut coin)| {
            coin.scene_param.description.clear();
            coin
        })
        .collect()
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

    let mut card_param = card.to_card_param(transform, runtime)?;
    card_param.scene_param = normalize_editor_card_scene_param(&CardSceneParam {
        instance_id: card_param.scene_param.instance_id.clone(),
        data: SceneParam {
            position: transform.translation.truncate(),
            rotation: transform.rotation.to_euler(EulerRot::XYZ).2,
            order: editor_local_order_from_transform(transform),
            enable_if: card_param.scene_param.data.enable_if.clone(),
            disable_if: card_param.scene_param.data.disable_if.clone(),
            description: String::new(),
        },
    });
    Some(card_param)
}

fn make_description(card_param: &CardParam, card_presets_config: &CardPresetsConfig) -> String {
    let appearance = card_param.load_appearance(card_presets_config);
    let prefab = card_param.load_prefab(card_presets_config);
    format!(
        "title: {}, image_path: {}{}",
        appearance.title,
        appearance.image_res_path,
        prefab
            .and_then(|p| { p.description })
            .map(|desc| { ", desc: ".to_string() + &desc })
            .unwrap_or_default()
    )
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
        (Without<EditorPlacedTerrain>, Without<EditorDragPreview>),
    >,
    terrain_query: &Query<
        (Entity, &Transform, &EditorTerrainData, Option<&Trap>),
        With<EditorPlacedTerrain>,
    >,
    character_coin_query: &Query<(Entity, &CharacterCoin, &Transform), With<EditorView>>,
    spawn_deps: &mut SpawnCardSystemParams<'_>,
    character_config: &CharacterConfig,
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
        commands.entity(entity).try_despawn();
    }
    for (entity, _, _, _) in terrain_query.iter() {
        commands.entity(entity).try_despawn();
    }
    for (entity, _, _) in character_coin_query.iter() {
        commands.entity(entity).try_despawn();
    }

    let cards = scene.cards;
    let terrains = scene.terrains;
    let character_coins = scene.character_coins;
    for card in &cards {
        let entity = spawn_editor_card(commands, spawn_deps, card);
        commands.entity(entity).insert(EditorPlacedCard);
    }
    for terrain in &terrains {
        spawn_editor_terrain(
            commands,
            terrain,
            spawn_deps.meshes.as_mut(),
            spawn_deps.materials.as_mut(),
        );
    }
    for character_coin in &character_coins {
        spawn_character_coin(
            commands,
            spawn_deps.asset_server.as_ref(),
            spawn_deps.meshes.as_mut(),
            spawn_deps.materials.as_mut(),
            &*spawn_deps.config,
            character_config,
            character_coin,
            EditorView,
        );
    }

    format!(
        "已从 {} 导入 {} 张卡牌、{} 个地形、{} 个角色硬币",
        path.display(),
        cards.len(),
        terrains.len(),
        character_coins.len()
    )
}

fn editor_scene_dir() -> PathBuf {
    runtime_root().join(EDITOR_SCENE_DIR)
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
    #[cfg(target_arch = "wasm32")]
    {
        let _ = current_path;
        return None;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
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
}

fn pick_scene_import_path(current_path: Option<&Path>) -> Option<PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = current_path;
        return None;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
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
