pub mod core;

use self::core::CoreGamePlugin;
use crate::action::ActionRegistrationExt;
use crate::editing::command::noise::CreateNoiseArea;
use crate::editing::command::{CommandSequence, EditorCommand};
use crate::editing::DoCommand;
use crate::hotkey::modifier::Modifier;
use crate::hotkey::Hotkey;
use crate::project::project_loaded;
use crate::selection::Selected;
use crate::settings::{AspectRatio, EditorSettings};
use crate::timing::ChartTime;
use crate::utils;
use crate::utils::convert::BevyEguiConvert;
use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy_persistent::Persistent;
use egui::Ui;
use phichain_chart::bpm_list::BpmList;
use phichain_chart::constants::{CANVAS_HEIGHT, CANVAS_WIDTH};
use phichain_chart::noise::{
    NoiseArea, NoiseMoveEvent, NoisePoint, NoiseRotateEvent, NoiseScaleEvent,
};

pub fn game_tab(In(mut ui): In<Ui>, world: &mut World) {
    let aspect_ratio = &world
        .resource::<Persistent<EditorSettings>>()
        .game
        .aspect_ratio;
    let clip_rect = ui.clip_rect();
    let viewport = match aspect_ratio {
        AspectRatio::Free => clip_rect,
        AspectRatio::Fixed { width, height } => {
            utils::misc::keep_aspect_ratio(clip_rect, width / height)
        }
    };

    let mut game_viewport = world.resource_mut::<GameViewport>();
    game_viewport.0 = viewport.into_bevy();

    let mut game_viewport = world.resource_mut::<phichain_game::GameViewport>();
    game_viewport.0 = viewport.into_bevy();

    noise_creation_ui(&mut ui, world, viewport);
    noise_resize_ui(&mut ui, world, viewport);
}

#[derive(Resource, Default)]
struct NoiseCreationTool {
    active: bool,
    start: Option<egui::Pos2>,
    subtract: bool,
}

#[derive(Resource)]
pub(crate) struct NoiseEditTool {
    pub auto_key: bool,
}

impl Default for NoiseEditTool {
    fn default() -> Self {
        Self { auto_key: true }
    }
}

#[derive(Resource, Default)]
struct NoiseResizeTool {
    drag: Option<(Entity, NoiseArea, NoiseHandle)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoiseHandle {
    Corner(usize),
    Rotate,
}

const NOISE_SNAP_GUIDES: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

fn snap_axis(value: f32, pixel_extent: f32) -> f32 {
    let threshold = 6.0 / pixel_extent.max(1.0);
    NOISE_SNAP_GUIDES
        .into_iter()
        .find(|guide| (value - guide).abs() <= threshold)
        .unwrap_or(value)
}

fn snap_point(point: NoisePoint, viewport: egui::Rect) -> NoisePoint {
    NoisePoint::new(
        snap_axis(point.x, viewport.width()),
        snap_axis(point.y, viewport.height()),
    )
}

fn noise_creation_ui(ui: &mut Ui, world: &mut World, viewport: egui::Rect) {
    let button_rect = egui::Rect::from_min_size(
        viewport.left_top() + egui::vec2(8.0, 8.0),
        egui::vec2(112.0, 28.0),
    );
    let active = world.resource::<NoiseCreationTool>().active;
    let button = ui.put(
        button_rect,
        egui::Button::new(t!("tab.game.draw_noise_area")).selected(active),
    );
    if button.clicked() {
        let mut tool = world.resource_mut::<NoiseCreationTool>();
        tool.active = !tool.active;
        tool.start = None;
    }

    let auto_key_rect = egui::Rect::from_min_size(
        egui::pos2(button_rect.right() + 4.0, button_rect.top()),
        egui::vec2(92.0, 28.0),
    );
    let auto_key = world.resource::<NoiseEditTool>().auto_key;
    if ui
        .put(
            auto_key_rect,
            egui::Button::new(t!("tab.game.auto_key")).selected(auto_key),
        )
        .clicked()
    {
        world.resource_mut::<NoiseEditTool>().auto_key = !auto_key;
    }

    let subtract_rect = egui::Rect::from_min_size(
        egui::pos2(auto_key_rect.right() + 4.0, button_rect.top()),
        egui::vec2(96.0, 28.0),
    );
    if world.resource::<NoiseCreationTool>().active {
        let subtract = world.resource::<NoiseCreationTool>().subtract;
        if ui
            .put(
                subtract_rect,
                egui::Button::new(t!("tab.game.subtract_domain")).selected(subtract),
            )
            .clicked()
        {
            world.resource_mut::<NoiseCreationTool>().subtract = !subtract;
        }
    }

    if !world.resource::<NoiseCreationTool>().active {
        return;
    }

    let draw_rect = viewport.with_min_y(button_rect.bottom() + 4.0);
    let response = ui.allocate_rect(draw_rect, egui::Sense::drag());
    if response.drag_started() {
        world.resource_mut::<NoiseCreationTool>().start = response.interact_pointer_pos();
    }

    if let (Some(start), Some(now)) = (
        world.resource::<NoiseCreationTool>().start,
        response.interact_pointer_pos(),
    ) {
        ui.painter().rect(
            egui::Rect::from_two_pos(start, now),
            0.0,
            egui::Color32::from_rgba_unmultiplied(255, 60, 70, 45),
            egui::Stroke::new(2.0_f32, egui::Color32::LIGHT_RED),
            egui::StrokeKind::Inside,
        );
    }

    if response.drag_stopped() {
        let start = world.resource_mut::<NoiseCreationTool>().start.take();
        let end = response.interact_pointer_pos();
        if let (Some(start), Some(end)) = (start, end) {
            let rect = egui::Rect::from_two_pos(start, end);
            if rect.width() >= 6.0 && rect.height() >= 6.0 {
                let to_percent = |position: egui::Pos2| {
                    snap_point(
                        NoisePoint {
                            x: ((position.x - viewport.left()) / viewport.width()).clamp(0.0, 1.0),
                            y: (1.0 - (position.y - viewport.top()) / viewport.height())
                                .clamp(0.0, 1.0),
                        },
                        viewport,
                    )
                };
                let bottom_left = to_percent(rect.left_bottom());
                let top_right = to_percent(rect.right_top());
                let current = world
                    .resource::<BpmList>()
                    .beat_at(world.resource::<ChartTime>().0);
                let mut area = NoiseArea::default();
                area.bottom_left_percentage = bottom_left;
                area.top_right_percentage = top_right;
                area.is_subtract = world.resource::<NoiseCreationTool>().subtract;
                area.appear_beat = current;
                area.enable_beat = current;
                area.disable_beat = current + phichain_chart::beat!(4);
                area.disappear_beat = area.disable_beat;
                world.write_message(DoCommand(EditorCommand::CreateNoiseArea(
                    CreateNoiseArea::new(area),
                )));
                // Drawing is a one-shot action. Leaving the creation tool
                // active suppresses all transform handles on the next frame,
                // which made a freshly drawn domain appear impossible to
                // rotate or resize.
                let mut tool = world.resource_mut::<NoiseCreationTool>();
                tool.active = false;
                tool.start = None;
            }
        }
    }
}

fn noise_resize_ui(ui: &mut Ui, world: &mut World, viewport: egui::Rect) {
    if world.resource::<NoiseCreationTool>().active {
        return;
    }
    let selected = world
        .query_filtered::<(Entity, &NoiseArea), With<Selected>>()
        .iter(world)
        .next()
        .map(|(entity, area)| (entity, area.clone()));
    let Some((entity, area)) = selected else {
        return;
    };
    let auto_key = world.resource::<NoiseEditTool>().auto_key;
    let seconds = world.resource::<ChartTime>().0;
    let bpm_list = world.resource::<BpmList>().clone();
    let beat = bpm_list.beat_at(seconds);
    let visual = area.rect_at(seconds, &bpm_list);
    let rotation = Vec2::from_angle(visual.rotation_degrees.to_radians());
    let corner_signs = [
        Vec2::new(-1.0, -1.0),
        Vec2::new(-1.0, 1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(1.0, 1.0),
    ];
    let world_to_screen = |point: Vec2| {
        egui::pos2(
            viewport.left() + (point.x / CANVAS_WIDTH + 0.5) * viewport.width(),
            viewport.top() + (0.5 - point.y / CANVAS_HEIGHT) * viewport.height(),
        )
    };
    let shift_down = ui.input(|input| input.modifiers.shift);

    for (corner, sign) in corner_signs.into_iter().enumerate() {
        let position = if auto_key {
            world_to_screen(visual.center + (sign * visual.size / 2.0).rotate(rotation))
        } else {
            let point = match corner {
                0 => area.bottom_left_percentage,
                1 => NoisePoint::new(area.bottom_left_percentage.x, area.top_right_percentage.y),
                2 => NoisePoint::new(area.top_right_percentage.x, area.bottom_left_percentage.y),
                _ => area.top_right_percentage,
            };
            world_to_screen(point.world())
        };
        let rect = egui::Rect::from_center_size(position, egui::vec2(14.0, 14.0));
        let response = ui.interact(
            rect,
            egui::Id::new(("noise-resize", entity, corner)),
            if shift_down {
                egui::Sense::hover()
            } else {
                egui::Sense::drag()
            },
        );
        ui.painter()
            .circle_filled(position, 5.0, egui::Color32::GREEN);
        if response.drag_started() {
            world.resource_mut::<NoiseResizeTool>().drag =
                Some((entity, area.clone(), NoiseHandle::Corner(corner)));
        }
        if response.dragged() {
            let Some((drag_entity, original, drag_handle)) =
                world.resource::<NoiseResizeTool>().drag.clone()
            else {
                continue;
            };
            if drag_entity != entity || drag_handle != NoiseHandle::Corner(corner) {
                continue;
            }
            let delta = response.drag_delta();
            if let Some(mut current) = world.get_mut::<NoiseArea>(entity) {
                *current = original;
                if auto_key {
                    let original_visual = current.rect_at(seconds, &bpm_list);
                    let raw_world_delta = Vec2::new(
                        delta.x / viewport.width() * CANVAS_WIDTH,
                        -delta.y / viewport.height() * CANVAS_HEIGHT,
                    );
                    let original_corner = original_visual.center
                        + (sign * original_visual.size / 2.0).rotate(Vec2::from_angle(
                            original_visual.rotation_degrees.to_radians(),
                        ));
                    let snapped_corner = snap_point(
                        NoisePoint::from_world(original_corner + raw_world_delta),
                        viewport,
                    )
                    .world();
                    let world_delta = snapped_corner - original_corner;
                    let local_delta = world_delta.rotate(Vec2::from_angle(
                        -original_visual.rotation_degrees.to_radians(),
                    ));
                    let desired_size = Vec2::new(
                        (original_visual.size.x + sign.x * local_delta.x).max(1.0),
                        (original_visual.size.y + sign.y * local_delta.y).max(1.0),
                    );
                    let base_size = Vec2::new(
                        (current.top_right_percentage.x - current.bottom_left_percentage.x).abs()
                            * CANVAS_WIDTH,
                        (current.top_right_percentage.y - current.bottom_left_percentage.y).abs()
                            * CANVAS_HEIGHT,
                    );
                    let opposite = original_visual.center
                        - (sign * original_visual.size / 2.0).rotate(Vec2::from_angle(
                            original_visual.rotation_degrees.to_radians(),
                        ));
                    let (ease_type_x, ease_type_y) = current
                        .scale_events
                        .iter()
                        .find(|key| key.beat == beat)
                        .map(|key| (key.ease_type_x, key.ease_type_y))
                        .unwrap_or((0, 0));
                    current.set_scale_key(NoiseScaleEvent {
                        beat,
                        anchor: NoisePoint::from_world(opposite),
                        scale: NoisePoint::new(
                            desired_size.x / base_size.x.max(1.0),
                            desired_size.y / base_size.y.max(1.0),
                        ),
                        ease_type_x,
                        ease_type_y,
                    });
                    let desired_center = original_visual.center
                        + (local_delta / 2.0).rotate(Vec2::from_angle(
                            original_visual.rotation_degrees.to_radians(),
                        ));
                    let (move_ease_x, move_ease_y) = current
                        .move_events
                        .iter()
                        .find(|key| key.beat == beat)
                        .map(|key| (key.ease_type_x, key.ease_type_y))
                        .unwrap_or((0, 0));
                    let end_position =
                        current.move_target_for_center(seconds, &bpm_list, desired_center);
                    current.set_move_key(NoiseMoveEvent {
                        beat,
                        end_position,
                        ease_type_x: move_ease_x,
                        ease_type_y: move_ease_y,
                    });
                } else {
                    let dx = delta.x / viewport.width();
                    let dy = -delta.y / viewport.height();
                    if corner == 0 || corner == 1 {
                        current.bottom_left_percentage.x = (current.bottom_left_percentage.x + dx)
                            .min(current.top_right_percentage.x - 0.001);
                    } else {
                        current.top_right_percentage.x = (current.top_right_percentage.x + dx)
                            .max(current.bottom_left_percentage.x + 0.001);
                    }
                    if corner == 0 || corner == 2 {
                        current.bottom_left_percentage.y = (current.bottom_left_percentage.y + dy)
                            .min(current.top_right_percentage.y - 0.001);
                    } else {
                        current.top_right_percentage.y = (current.top_right_percentage.y + dy)
                            .max(current.bottom_left_percentage.y + 0.001);
                    }
                }
            }
        }
        if response.drag_stopped() {
            let drag = world.resource_mut::<NoiseResizeTool>().drag.take();
            if let Some((drag_entity, from, _)) = drag {
                if let Some(to) = world.get::<NoiseArea>(drag_entity).cloned() {
                    if from != to {
                        world.write_message(DoCommand(EditorCommand::EditNoiseArea(
                            crate::editing::command::noise::EditNoiseArea {
                                entity: drag_entity,
                                from,
                                to,
                            },
                        )));
                    }
                }
            }
        }
    }

    // Rotation is always a keyframed transform because the official format has no base-rotation
    // field. Place the handle above the currently rendered rectangle, including its animation.
    let center = world_to_screen(visual.center);
    let top = world_to_screen(
        visual.center
            + Vec2::new(0.0, visual.size.y / 2.0)
                .rotate(Vec2::from_angle(visual.rotation_degrees.to_radians())),
    );
    let bottom = world_to_screen(
        visual.center
            - Vec2::new(0.0, visual.size.y / 2.0)
                .rotate(Vec2::from_angle(visual.rotation_degrees.to_radians())),
    );
    let outward = (top - center).normalized();
    let top_candidate = top + outward * 24.0;
    let bottom_candidate = bottom - outward * 24.0;
    let usable_viewport = viewport
        .shrink2(egui::vec2(10.0, 10.0))
        .with_min_y(viewport.top() + 42.0);
    // Prefer the conventional handle above the domain. If the toolbar or a
    // preview edge leaves no room there, flip it below instead of clamping it
    // into the domain where it becomes indistinguishable from the content.
    let (handle_edge, desired_rotate_position) = if usable_viewport.contains(top_candidate) {
        (top, top_candidate)
    } else {
        (bottom, bottom_candidate)
    };
    let rotate_position = egui::pos2(
        desired_rotate_position
            .x
            .clamp(usable_viewport.left(), usable_viewport.right()),
        desired_rotate_position
            .y
            .clamp(usable_viewport.top(), usable_viewport.bottom()),
    );
    ui.painter().line_segment(
        [handle_edge, rotate_position],
        egui::Stroke::new(1.5_f32, egui::Color32::LIGHT_BLUE),
    );
    let response = ui
        .interact(
            egui::Rect::from_center_size(rotate_position, egui::vec2(20.0, 20.0)),
            egui::Id::new(("noise-rotate", entity)),
            if shift_down {
                egui::Sense::hover()
            } else {
                egui::Sense::drag()
            },
        )
        .on_hover_text(t!("tab.inspector.noise_area.add_rotate"));
    ui.painter()
        .circle_filled(rotate_position, 6.0, egui::Color32::LIGHT_BLUE);
    if response.drag_started() {
        world.resource_mut::<NoiseResizeTool>().drag =
            Some((entity, area.clone(), NoiseHandle::Rotate));
    }
    if response.dragged() {
        let drag = world.resource::<NoiseResizeTool>().drag.clone();
        if let (Some((drag_entity, original, NoiseHandle::Rotate)), Some(pointer)) =
            (drag, response.interact_pointer_pos())
        {
            if drag_entity == entity {
                let direction = Vec2::new(pointer.x - center.x, center.y - pointer.y);
                if direction.length_squared() > 1.0 {
                    let raw_rotation = direction.y.atan2(direction.x).to_degrees() - 90.0;
                    let nearest_step = (raw_rotation / 15.0).round() * 15.0;
                    let rotation_degrees = if (raw_rotation - nearest_step).abs() <= 3.0 {
                        nearest_step
                    } else {
                        raw_rotation
                    };
                    let ease_type = original
                        .rotate_events
                        .iter()
                        .find(|key| key.beat == beat)
                        .map(|key| key.ease_type)
                        .unwrap_or(0);
                    if let Some(mut current) = world.get_mut::<NoiseArea>(entity) {
                        *current = original;
                        current.set_rotate_key(NoiseRotateEvent {
                            beat,
                            anchor: NoisePoint::from_world(visual.center),
                            rotation: rotation_degrees,
                            ease_type,
                        });
                        let (move_ease_x, move_ease_y) = current
                            .move_events
                            .iter()
                            .find(|key| key.beat == beat)
                            .map(|key| (key.ease_type_x, key.ease_type_y))
                            .unwrap_or((0, 0));
                        let end_position =
                            current.move_target_for_center(seconds, &bpm_list, visual.center);
                        current.set_move_key(NoiseMoveEvent {
                            beat,
                            end_position,
                            ease_type_x: move_ease_x,
                            ease_type_y: move_ease_y,
                        });
                    }
                }
            }
        }
    }
    if response.drag_stopped() {
        let drag = world.resource_mut::<NoiseResizeTool>().drag.take();
        if let Some((drag_entity, from, NoiseHandle::Rotate)) = drag {
            if let Some(to) = world.get::<NoiseArea>(drag_entity).cloned() {
                if from != to {
                    world.write_message(DoCommand(EditorCommand::EditNoiseArea(
                        crate::editing::command::noise::EditNoiseArea {
                            entity: drag_entity,
                            from,
                            to,
                        },
                    )));
                }
            }
        }
    }
}

pub struct GameTabPlugin;

impl Plugin for GameTabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameViewport(Rect::from_corners(Vec2::ZERO, Vec2::ZERO)))
            .init_resource::<NoiseCreationTool>()
            .init_resource::<NoiseEditTool>()
            .init_resource::<NoiseResizeTool>()
            .add_systems(
                PostUpdate,
                update_game_camera_viewport_system.run_if(project_loaded()),
            )
            .add_action(
                "phichain.duplicate_noise_area",
                duplicate_selected_noise_areas,
                Some(Hotkey::new(KeyCode::KeyD, vec![Modifier::Control])),
            )
            .add_plugins(CoreGamePlugin);
    }
}

fn duplicate_selected_noise_areas(
    areas: Query<&NoiseArea, With<Selected>>,
    mut commands: MessageWriter<DoCommand>,
) -> Result {
    let sequence = areas
        .iter()
        .cloned()
        .map(|area| EditorCommand::CreateNoiseArea(CreateNoiseArea::new(area)))
        .collect::<Vec<_>>();
    if !sequence.is_empty() {
        commands.write(DoCommand(EditorCommand::CommandSequence(CommandSequence(
            sequence,
        ))));
    }
    Ok(())
}

#[derive(Resource, Debug)]
pub struct GameViewport(pub Rect);

#[derive(Component)]
pub struct GameCamera;

pub fn update_game_camera_viewport_system(
    mut query: Query<&mut Camera, With<GameCamera>>,
    window_query: Query<&Window>,
    egui_settings: Query<&bevy_egui::EguiContextSettings>,
    game_viewport: Res<GameViewport>,
) -> Result {
    let mut game_camera = query.single_mut()?;
    let Ok(window) = window_query.single() else {
        return Ok(());
    };

    let scale_factor = window.scale_factor() * egui_settings.single()?.scale_factor;
    let viewport_pos = game_viewport.0.min * scale_factor;
    let viewport_size = game_viewport.0.size() * scale_factor;

    if viewport_pos.x < 0.0
        || viewport_pos.y < 0.0
        || viewport_size.x <= 0.0
        || viewport_size.y <= 0.0
        || viewport_pos.x + viewport_size.x > window.width() * scale_factor
        || viewport_pos.y + viewport_size.y > window.height() * scale_factor
    {
        game_camera.viewport = None;
        return Ok(());
    }

    game_camera.viewport = Some(Viewport {
        physical_position: viewport_pos.as_uvec2(),
        physical_size: viewport_size.as_uvec2(),
        depth: 0.0..1.0,
    });

    Ok(())
}
