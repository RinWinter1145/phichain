use crate::editing::command::noise::EditNoiseArea;
use crate::editing::command::EditorCommand;
use crate::editing::DoCommand;
use crate::selection::{Select, Selected, SelectedLine};
use crate::tab::game::{GameViewport, NoiseEditTool};
use crate::timing::ChartTime;
use bevy::prelude::*;
use phichain_chart::bpm_list::BpmList;
use phichain_chart::line::{Line, LineOpacity};
use phichain_chart::noise::{NoiseArea, NoiseMoveEvent, NoisePoint};
use phichain_chart::note::Note;
use phichain_game::noise::NoiseAreaOrder;

#[derive(Resource, Default)]
struct NoiseDrag {
    pointer_entity: Option<Entity>,
    area: Option<(Entity, NoiseArea)>,
}

fn snap_noise_point(point: NoisePoint, viewport: Rect) -> NoisePoint {
    let snap = |value: f32, extent: f32| {
        let threshold = 6.0 / extent.max(1.0);
        [0.0, 0.25, 0.5, 0.75, 1.0]
            .into_iter()
            .find(|guide| (value - guide).abs() <= threshold)
            .unwrap_or(value)
    };
    NoisePoint::new(
        snap(point.x, viewport.width()),
        snap(point.y, viewport.height()),
    )
}

pub struct PickingPlugin;

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NoiseDrag>()
            .add_observer(on_click_note)
            .add_observer(on_click_line)
            .add_observer(on_click_noise_area)
            .add_observer(on_drag_start_noise_area)
            .add_observer(on_drag_noise_area)
            .add_observer(on_drag_end_noise_area);
    }
}

fn on_click_noise_area(
    mut click: On<Pointer<Click>>,
    areas: Query<(Entity, &NoiseArea, &NoiseAreaOrder, Option<&Selected>)>,
    viewport: Res<GameViewport>,
    bpm_list: Option<Res<BpmList>>,
    chart_time: Res<ChartTime>,
    mut select: MessageWriter<Select>,
) {
    if click.button != PointerButton::Primary || areas.get(click.entity).is_err() {
        return;
    }
    let Some(bpm_list) = bpm_list.as_deref() else {
        return;
    };
    let Some(hit) = click.hit.position else {
        return;
    };
    let scale = Vec2::new(
        viewport.0.width() / phichain_chart::constants::CANVAS_WIDTH,
        viewport.0.height() / phichain_chart::constants::CANVAS_HEIGHT,
    );
    if scale.x == 0.0 || scale.y == 0.0 {
        return;
    }
    let point = Vec2::new(hit.x / scale.x, hit.y / scale.y);
    let mut candidates = areas
        .iter()
        .filter(|(_, area, _, _)| area.rect_at(chart_time.0, bpm_list).contains(point))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(_, _, order, _)| std::cmp::Reverse(order.0));
    let next = candidates
        .iter()
        .position(|(_, _, _, selected)| selected.is_some())
        .map(|index| (index + 1) % candidates.len())
        .unwrap_or(0);
    if let Some((entity, _, _, _)) = candidates.get(next) {
        select.write(Select(vec![*entity]));
    }
    click.propagate(false);
}

fn on_drag_start_noise_area(
    event: On<Pointer<DragStart>>,
    keys: Res<ButtonInput<KeyCode>>,
    areas: Query<(Entity, &NoiseArea, Option<&Selected>)>,
    viewport: Res<GameViewport>,
    bpm_list: Option<Res<BpmList>>,
    chart_time: Res<ChartTime>,
    mut drag: ResMut<NoiseDrag>,
    mut select: MessageWriter<Select>,
) {
    let shift_pressed = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if event.button != PointerButton::Primary || !shift_pressed {
        return;
    }
    let Some(bpm_list) = bpm_list.as_deref() else {
        return;
    };
    let Ok((_, picked, _)) = areas.get(event.entity) else {
        return;
    };
    let scale = Vec2::new(
        viewport.0.width() / phichain_chart::constants::CANVAS_WIDTH,
        viewport.0.height() / phichain_chart::constants::CANVAS_HEIGHT,
    );
    let hit = event.hit.position.map(|hit| {
        Vec2::new(
            hit.x / scale.x.max(f32::EPSILON),
            hit.y / scale.y.max(f32::EPSILON),
        )
    });
    let selected = hit.and_then(|point| {
        areas
            .iter()
            .find(|(_, area, selected)| {
                selected.is_some() && area.rect_at(chart_time.0, bpm_list).contains(point)
            })
            .map(|(entity, area, _)| (entity, area.clone()))
    });
    let (entity, area) = selected.unwrap_or((event.entity, picked.clone()));
    drag.pointer_entity = Some(event.entity);
    drag.area = Some((entity, area));
    if entity == event.entity {
        select.write(Select(vec![entity]));
    }
}

fn on_drag_noise_area(
    event: On<Pointer<Drag>>,
    mut areas: Query<&mut NoiseArea>,
    drag: Res<NoiseDrag>,
    viewport: Res<GameViewport>,
    tool: Res<NoiseEditTool>,
    bpm_list: Option<Res<BpmList>>,
    chart_time: Res<ChartTime>,
) {
    if drag.pointer_entity != Some(event.entity) {
        return;
    }
    let Some(bpm_list) = bpm_list.as_deref() else {
        return;
    };
    let Some((entity, original)) = drag.area.as_ref() else {
        return;
    };
    if viewport.0.width() <= 0.0 || viewport.0.height() <= 0.0 {
        return;
    }
    if let Ok(mut area) = areas.get_mut(*entity) {
        let dx = event.distance.x / viewport.0.width();
        let dy = -event.distance.y / viewport.0.height();
        *area = original.clone();
        if tool.auto_key {
            let beat = bpm_list.beat_at(chart_time.0);
            let original_center = original.rect_at(chart_time.0, bpm_list).center;
            let desired_center = snap_noise_point(
                NoisePoint::from_world(
                    original_center
                        + Vec2::new(
                            dx * phichain_chart::constants::CANVAS_WIDTH,
                            dy * phichain_chart::constants::CANVAS_HEIGHT,
                        ),
                ),
                viewport.0,
            )
            .world();
            let (ease_type_x, ease_type_y) = original
                .move_events
                .iter()
                .find(|key| key.beat == beat)
                .map(|key| (key.ease_type_x, key.ease_type_y))
                .unwrap_or((0, 0));
            area.set_move_key(NoiseMoveEvent {
                beat,
                end_position: original.move_target_for_center(
                    chart_time.0,
                    bpm_list,
                    desired_center,
                ),
                ease_type_x,
                ease_type_y,
            });
        } else {
            let center = NoisePoint::new(
                (original.bottom_left_percentage.x + original.top_right_percentage.x) / 2.0,
                (original.bottom_left_percentage.y + original.top_right_percentage.y) / 2.0,
            );
            let target =
                snap_noise_point(NoisePoint::new(center.x + dx, center.y + dy), viewport.0);
            let snapped_dx = target.x - center.x;
            let snapped_dy = target.y - center.y;
            area.bottom_left_percentage.x += snapped_dx;
            area.bottom_left_percentage.y += snapped_dy;
            area.top_right_percentage.x += snapped_dx;
            area.top_right_percentage.y += snapped_dy;
        }
    }
}

fn on_drag_end_noise_area(
    event: On<Pointer<DragEnd>>,
    areas: Query<&NoiseArea>,
    mut drag: ResMut<NoiseDrag>,
    mut edit: MessageWriter<DoCommand>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    if drag.pointer_entity.take() != Some(event.entity) {
        drag.area = None;
        return;
    }
    let Some((entity, from)) = drag.area.take() else {
        return;
    };
    if let Ok(to) = areas.get(entity) {
        if from != *to {
            edit.write(DoCommand(EditorCommand::EditNoiseArea(EditNoiseArea {
                entity,
                from,
                to: to.clone(),
            })));
        }
    }
}

fn on_click_note(
    mut click: On<Pointer<Click>>,
    note_query: Query<(), With<Note>>,
    mut select: MessageWriter<Select>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    if note_query.contains(click.entity) {
        select.write(Select(vec![click.entity]));
        click.propagate(false); // Don't bubble to parent Line
    }
}

fn on_click_line(
    click: On<Pointer<Click>>,
    line_query: Query<&LineOpacity, With<Line>>,
    selected_line: Option<ResMut<SelectedLine>>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    if let Some(mut selected_line) = selected_line {
        if let Ok(opacity) = line_query.get(click.entity) {
            if opacity.0 > 0.0 {
                selected_line.0 = click.entity;
            }
        }
    }
}
