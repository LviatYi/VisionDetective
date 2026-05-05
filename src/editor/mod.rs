use crate::AppView;
use crate::card::card_params::{
    CardParam, CardRuntimeSpecializedConfig, CardSceneParam, CardSpawnParams,
};
use crate::card::specialized::clue::ClueCardParams;
use crate::card::{CARD_SIZE, Card, spawn_card_by_card_param};
use crate::config::GameConfig;
use crate::config::card_config::CardPresetsConfig;
use crate::editor::editor_view::{EditorView, setup_editor_view};
use crate::game_view::main_view::cleanup_view;
use bevy::input::ButtonInput;
use bevy::picking::pointer::PointerButton;
use bevy::picking::prelude::{Drag, Move, Out, Over, Pointer, Press, Release, Scroll};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::{PrimaryWindow, Window};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::VecDeque;
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
const CARD_ORDER_STEP: f32 = 1.0;
const DRAG_PREVIEW_ORDER: f32 = 20.0;
const ORDER_LABEL_FONT_SIZE: f32 = 13.0;
const CLUE_PREFAB_ID: u32 = 1006;
const DEFAULT_CLUE_INTERACTION_PREFAB_ID: u32 = 1003;
const DEFAULT_CLUE_TARGET_OFFSET: Vec2 = Vec2::new(0.0, CARD_SIZE.y * 0.4);
const CLUE_LINK_DASH_LENGTH: f32 = 18.0;
const CLUE_LINK_GAP_LENGTH: f32 = 10.0;

pub struct EditorPlugin;

#[derive(Resource, Default)]
struct EditorPointerWorldPosition(Option<Vec2>);

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

#[derive(Component)]
struct EditorCardOrderText;

#[derive(Component, Clone, Copy)]
struct EditorClueLink {
    target: Entity,
}

#[derive(Component, Clone, Copy)]
struct EditorClueTargetCard {
    clue: Entity,
}

struct PrefabPreviewItem {
    id: u32,
    title: String,
    background_color: Color,
}

#[derive(Serialize, Deserialize, Default)]
struct EditorSceneFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<CardParam>,
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorInteractionState>()
            .init_resource::<EditorPointerWorldPosition>()
            .add_systems(OnEnter(AppView::Editor), reset_editor_state)
            .add_systems(OnEnter(AppView::Editor), setup_editor_view)
            .add_systems(OnEnter(AppView::Editor), setup_editor_ui)
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
                    handle_scene_editing,
                    handle_card_order_wheel,
                    handle_card_order_shortcuts,
                    update_editor_card_order_text,
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
                            "操作说明\n1. 直接按住卡牌预览并拖到主场景，松开后会克隆一张。\n2. 左键拖动卡牌位置。\n3. 拖动右上角旋转控制点可直接旋转。\n4. 鼠标悬浮卡牌时滚轮调整其在相交卡牌组内的层级。\n5. PageUp / PageDown 调整层级，Home / End 置底或置顶。\n6. Delete 删除选中卡牌。\n7. Ctrl+E / Ctrl+B 导出，Ctrl+I / Ctrl+Shift+I 导入。",
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
            Option<&EditorClueLink>,
            Option<&EditorClueTargetCard>,
        ),
        Without<EditorDragPreview>,
    >,
    mut spawn_deps: CardSpawnParams<'_>,
    mut state: ResMut<EditorInteractionState>,
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

    let Ok((mut transform, mut visibility)) = preview_query.get_mut(preview_entity) else {
        return;
    };

    transform.translation.x = world_position.x;
    transform.translation.y = world_position.y;
    transform.translation.z = DRAG_PREVIEW_ORDER;
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
    card_query: Query<&Transform, (With<Card>, Without<EditorDragPreview>)>,
    mut spawn_deps: CardSpawnParams<'_>,
    mut state: ResMut<EditorInteractionState>,
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

    let Some(entity) = preview_entity else {
        state.status_message = "放置卡牌失败：拖拽预览不存在".into();
        return;
    };

    let placed_order = next_card_order(&card_query);
    if let Ok(mut transform) = preview_query.get_mut(entity) {
        transform.translation.x = world_position.x;
        transform.translation.y = world_position.y;
        transform.translation.z = placed_order;
        transform.rotation = Quat::IDENTITY;
    }

    commands
        .entity(entity)
        .remove::<EditorDragPreview>()
        .insert(Visibility::Visible);

    if prefab_id == CLUE_PREFAB_ID {
        let target_position = world_position + DEFAULT_CLUE_TARGET_OFFSET;
        let target_order = placed_order + CARD_ORDER_STEP;
        let target_entity = spawn_editor_card(
            &mut commands,
            &mut spawn_deps,
            &CardParam {
                scene_param: CardSceneParam {
                    position: target_position,
                    rotation: 0.0,
                    order: target_order,
                },
                prefab_id: DEFAULT_CLUE_INTERACTION_PREFAB_ID,
                runtime_specialized_param: None,
            },
        );
        commands.entity(entity).insert(EditorClueLink {
            target: target_entity,
        });
        commands
            .entity(target_entity)
            .insert(EditorClueTargetCard { clue: entity });
        state.selected_entity = Some(target_entity);
        state.status_message = format!(
            "已放置线索卡 #{}，并创建目标交互卡 #{}",
            prefab_id, DEFAULT_CLUE_INTERACTION_PREFAB_ID
        );
        return;
    }

    state.selected_entity = Some(entity);
    state.status_message = format!(
        "已放置卡牌 #{} ({:.0}, {:.0})，层级 {:.0}",
        prefab_id, world_position.x, world_position.y, placed_order
    );
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
    )>,
    clue_link_query: Query<&EditorClueLink>,
    clue_target_query: Query<&EditorClueTargetCard>,
    mut state: ResMut<EditorInteractionState>,
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

            if let Ok((entity, _, _, transform)) = card_query.get(event.entity) {
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

    for event in drag_events.read() {
        if event.button != PointerButton::Primary {
            continue;
        }
        let Some(cursor_world_position) =
            cursor_world_position(&camera_query, event.pointer_location.position)
        else {
            continue;
        };
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

    let released = release_events
        .read()
        .any(|event| event.button == PointerButton::Primary);
    if released {
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
        despawn_editor_card_with_clue_links(
            &mut commands,
            entity,
            &clue_link_query,
            &clue_target_query,
        );
        state.moving_entity = None;
        state.rotating_entity = None;
        state.status_message = "已删除选中卡牌".into();
    }
}

fn despawn_editor_card_with_clue_links(
    commands: &mut Commands,
    entity: Entity,
    clue_link_query: &Query<&EditorClueLink>,
    clue_target_query: &Query<&EditorClueTargetCard>,
) {
    if let Ok(link) = clue_link_query.get(entity) {
        commands.entity(link.target).despawn();
    }
    if let Ok(target) = clue_target_query.get(entity) {
        commands.entity(target.clue).despawn();
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
            Option<&EditorClueLink>,
            Option<&EditorClueTargetCard>,
        ),
        Without<EditorDragPreview>,
    >,
    spawn_deps: CardSpawnParams<'_>,
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

fn handle_card_order_shortcuts(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut card_query: Query<(Entity, &mut Transform), (With<Card>, Without<EditorDragPreview>)>,
    mut state: ResMut<EditorInteractionState>,
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
        max_card_order_mut(&card_query).map(|order| CardOrderChange::Set(order + CARD_ORDER_STEP))
    } else if keyboard_input.just_pressed(KeyCode::Home) {
        min_card_order_mut(&card_query).map(|order| CardOrderChange::Set(order - CARD_ORDER_STEP))
    } else {
        None
    };

    let Some(order_delta) = order_delta else {
        return;
    };

    let Ok((_, mut transform)) = card_query.get_mut(selected_entity) else {
        state.selected_entity = None;
        state.status_message = "调整层级失败：选中卡牌不存在".into();
        return;
    };

    match order_delta {
        CardOrderChange::Step(delta) => transform.translation.z += delta,
        CardOrderChange::Set(order) => transform.translation.z = order,
    }

    state.status_message = format!("卡牌层级已更新为 {:.0}", transform.translation.z);
}

fn handle_card_order_wheel(
    mut scroll_events: MessageReader<Pointer<Scroll>>,
    mut card_queries: ParamSet<(
        Query<(Entity, &Card, &GlobalTransform, &Transform), Without<EditorDragPreview>>,
        Query<&mut Transform, (With<Card>, Without<EditorDragPreview>)>,
    )>,
    mut state: ResMut<EditorInteractionState>,
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

    {
        let mut transform_query = card_queries.p1();
        for (entity, order) in assignments {
            if let Ok(mut transform) = transform_query.get_mut(entity) {
                transform.translation.z = order;
            }
        }
    }

    state.selected_entity = Some(hovered_entity);
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
        **text = format!("order {:.0}", transform.translation.z);
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
    clue_link_query: Query<(Entity, &EditorClueLink)>,
) {
    for (clue_entity, link) in &clue_link_query {
        let Ok((_, clue_transform)) = card_query.get(clue_entity) else {
            continue;
        };
        let Ok((_, target_transform)) = card_query.get(link.target) else {
            continue;
        };
        draw_dashed_line(
            &mut gizmos,
            clue_transform.translation.truncate(),
            target_transform.translation.truncate(),
            Color::srgb(0.95, 0.82, 0.35),
        );
    }

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

fn draw_dashed_line(gizmos: &mut Gizmos, start: Vec2, end: Vec2, color: Color) {
    let delta = end - start;
    let distance = delta.length();
    if distance <= f32::EPSILON {
        return;
    }

    let direction = delta / distance;
    let step = CLUE_LINK_DASH_LENGTH + CLUE_LINK_GAP_LENGTH;
    let mut cursor = 0.0;
    while cursor < distance {
        let dash_end = (cursor + CLUE_LINK_DASH_LENGTH).min(distance);
        gizmos.line_2d(
            start + direction * cursor,
            start + direction * dash_end,
            color,
        );
        cursor += step;
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

#[derive(Clone)]
struct CardOrderSnapshot {
    entity: Entity,
    corners: [Vec2; 4],
    order: f32,
}

fn next_card_order<F: bevy::ecs::query::QueryFilter>(query: &Query<&Transform, F>) -> f32 {
    query
        .iter()
        .map(|transform| transform.translation.z)
        .reduce(f32::max)
        .unwrap_or(0.0)
        + CARD_ORDER_STEP
}

fn min_card_order_mut<F: bevy::ecs::query::QueryFilter>(
    query: &Query<(Entity, &mut Transform), F>,
) -> Option<f32> {
    query
        .iter()
        .map(|(_, transform)| transform.translation.z)
        .reduce(f32::min)
}

fn max_card_order_mut<F: bevy::ecs::query::QueryFilter>(
    query: &Query<(Entity, &mut Transform), F>,
) -> Option<f32> {
    query
        .iter()
        .map(|(_, transform)| transform.translation.z)
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
            order: transform.translation.z,
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

fn spawn_editor_card(
    commands: &mut Commands,
    spawn_deps: &mut CardSpawnParams<'_>,
    card_param: &CardParam,
) -> Entity {
    let entity = spawn_card_by_card_param(commands, spawn_deps, card_param);
    commands.entity(entity).insert(EditorView);
    append_editor_card_overlays(
        commands,
        entity,
        &spawn_deps.asset_server,
        &spawn_deps.config,
    );
    entity
}

fn spawn_editor_scene_card(
    commands: &mut Commands,
    spawn_deps: &mut CardSpawnParams<'_>,
    card_param: &CardParam,
) -> Entity {
    let clue_target = clue_target_from_scene_card(card_param);
    let clue_entity = spawn_editor_card(commands, spawn_deps, card_param);
    let Some((target_prefab_id, target_scene_param)) = clue_target else {
        return clue_entity;
    };

    let target_entity = spawn_editor_card(
        commands,
        spawn_deps,
        &CardParam {
            scene_param: target_scene_param,
            prefab_id: target_prefab_id,
            runtime_specialized_param: None,
        },
    );
    commands.entity(clue_entity).insert(EditorClueLink {
        target: target_entity,
    });
    commands
        .entity(target_entity)
        .insert(EditorClueTargetCard { clue: clue_entity });
    clue_entity
}

fn clue_target_from_scene_card(card_param: &CardParam) -> Option<(u32, CardSceneParam)> {
    let runtime = card_param.runtime_specialized_param.as_ref()?;
    if runtime.type_id != "clue" {
        return None;
    }

    let params = serde_json::from_value::<ClueCardParams>(runtime.params.clone()).ok()?;
    if params.interaction_prefab_id == 0 {
        None
    } else {
        Some((
            params.interaction_prefab_id,
            params.interaction_target_scene_param,
        ))
    }
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

#[derive(Clone, Copy)]
enum SceneFileFormat {
    Toml,
    Binary,
}

fn save_scene(
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorClueLink>,
            Option<&EditorClueTargetCard>,
        ),
        Without<EditorDragPreview>,
    >,
    format: SceneFileFormat,
) -> String {
    save_scene_to_path(card_query, &scene_path(format))
}

fn save_scene_to_path(
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorClueLink>,
            Option<&EditorClueTargetCard>,
        ),
        Without<EditorDragPreview>,
    >,
    path: &Path,
) -> String {
    let mut cards = card_query
        .iter()
        .filter_map(|(entity, card, transform, link, target)| {
            editor_card_to_scene_card(entity, card, transform, link, target, card_query)
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
        cards: cards.into_iter().map(|(_, card)| card).collect(),
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

fn editor_card_to_scene_card(
    _entity: Entity,
    card: &Card,
    transform: &Transform,
    link: Option<&EditorClueLink>,
    target: Option<&EditorClueTargetCard>,
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorClueLink>,
            Option<&EditorClueTargetCard>,
        ),
        Without<EditorDragPreview>,
    >,
) -> Option<CardParam> {
    if target.is_some() {
        return None;
    }

    let mut card_param = card.to_card_param(transform);
    let Some(link) = link else {
        return Some(card_param);
    };

    let Ok((_, target_card, target_transform, _, _)) = card_query.get(link.target) else {
        return Some(card_param);
    };

    card_param.runtime_specialized_param = Some(CardRuntimeSpecializedConfig {
        type_id: "clue".to_string(),
        params: json!({
            "interaction_prefab_id": target_card.instance_type.get_prefab_id(),
            "interaction_target_scene_param": target_card.to_card_param(target_transform).scene_param,
        }),
    });
    Some(card_param)
}

fn load_scene(
    commands: &mut Commands,
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorClueLink>,
            Option<&EditorClueTargetCard>,
        ),
        Without<EditorDragPreview>,
    >,
    mut spawn_deps: CardSpawnParams<'_>,
    format: SceneFileFormat,
) -> String {
    load_scene_from_path(commands, card_query, &mut spawn_deps, &scene_path(format))
}

fn load_scene_from_path(
    commands: &mut Commands,
    card_query: &Query<
        (
            Entity,
            &Card,
            &Transform,
            Option<&EditorClueLink>,
            Option<&EditorClueTargetCard>,
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
        SceneFileFormat::Binary => read_scene_binary(&path).map_err(|error| error.to_string()),
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
        spawn_editor_scene_card(commands, spawn_deps, card);
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
    if scene
        .cards
        .iter()
        .any(|card| card.runtime_specialized_param.is_some())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "二进制场景格式暂不支持 runtime_specialized_param，请导出 TOML",
        ));
    }

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"VDES");
    bytes.extend_from_slice(&(scene.cards.len() as u32).to_le_bytes());

    for card in &scene.cards {
        bytes.extend_from_slice(&card.prefab_id.to_le_bytes());
        bytes.extend_from_slice(&card.scene_param.position.x.to_le_bytes());
        bytes.extend_from_slice(&card.scene_param.position.y.to_le_bytes());
        bytes.extend_from_slice(&card.scene_param.rotation.to_le_bytes());
        bytes.extend_from_slice(&card.scene_param.order.to_le_bytes());
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

    if bytes.len() == 8 + count * 20 {
        let mut cards = Vec::with_capacity(count);
        for _ in 0..count {
            let prefab_id = read_u32(&bytes, &mut cursor)?;
            let position = Vec2::new(
                read_f32(&bytes, &mut cursor)?,
                read_f32(&bytes, &mut cursor)?,
            );
            let rotation = read_f32(&bytes, &mut cursor)?;
            let order = read_f32(&bytes, &mut cursor)?;
            cards.push(CardParam {
                scene_param: CardSceneParam {
                    position,
                    rotation,
                    order,
                },
                prefab_id,
                runtime_specialized_param: None,
            });
        }

        return Ok(EditorSceneFile { cards });
    }

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
                scene_param: CardSceneParam {
                    position,
                    rotation,
                    order: 0.0,
                },
                prefab_id,
                runtime_specialized_param: None,
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
