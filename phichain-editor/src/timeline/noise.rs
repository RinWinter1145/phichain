use crate::editing::command::noise::EditNoiseArea;
use crate::editing::command::EditorCommand;
use crate::editing::DoCommand;
use crate::selection::{Select, Selected};
use crate::timeline::{Timeline, TimelineContext};
use crate::ui::widgets::beat_range_drag_zone::{BeatRangeDragZone, TimelineBeatRange};
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui};
use phichain_chart::beat::Beat;
use phichain_chart::bpm_list::BpmList;
use phichain_chart::noise::NoiseArea;
use phichain_game::noise::NoiseAreaOrder;

#[derive(Debug, Clone, Default)]
pub struct NoiseTimeline;

#[derive(Clone, PartialEq)]
struct ActiveRange {
    start: Beat,
    end: Beat,
}

impl TimelineBeatRange for ActiveRange {
    fn start_beat_value(&self) -> f32 {
        self.start.value()
    }

    fn end_beat_value(&self) -> f32 {
        self.end.value()
    }

    fn set_start_beat(&mut self, beat: Beat) {
        self.start = beat;
    }

    fn set_end_beat(&mut self, beat: Beat) {
        self.end = beat;
    }
}

impl Timeline for NoiseTimeline {
    fn ui(&self, ui: &mut Ui, world: &mut World, viewport: Rect) {
        let mut state: SystemState<(
            TimelineContext,
            Query<(Entity, &mut NoiseArea, &NoiseAreaOrder, Option<&Selected>)>,
            Res<BpmList>,
            MessageWriter<Select>,
            MessageWriter<DoCommand>,
        )> = SystemState::new(world);
        let (ctx, mut areas, _bpm_list, mut select, mut edit) = state.get_mut(world);

        let count = areas.iter().count().max(1);
        let columns = count.min(8);
        let column_width = viewport.width() / columns as f32;

        for (entity, mut area, order, selected) in &mut areas {
            let x = viewport.left() + column_width * (order.0 % columns) as f32;
            let outer = beat_rect(&ctx, x, column_width, area.appear_beat, area.disappear_beat);
            let inner = beat_rect(
                &ctx,
                x + 3.0,
                (column_width - 6.0).max(2.0),
                area.enable_beat,
                area.disable_beat,
            );
            let base = if area.is_subtract {
                Color32::from_rgb(80, 190, 255)
            } else {
                Color32::from_rgb(255, 76, 82)
            };
            let stroke = if selected.is_some() {
                Stroke::new(2.0_f32, Color32::LIGHT_GREEN)
            } else {
                Stroke::new(1.0_f32, base)
            };

            let response = ui.allocate_rect(outer, Sense::click());
            ui.painter().rect(
                outer,
                2.0,
                base.gamma_multiply(0.18),
                stroke,
                StrokeKind::Inside,
            );
            ui.painter()
                .rect_filled(inner, 1.0, base.gamma_multiply(0.55));
            ui.painter().text(
                outer.center_top() + egui::vec2(0.0, 2.0),
                Align2::CENTER_TOP,
                order.0 + 1,
                FontId::monospace(10.0),
                Color32::WHITE,
            );

            if response.clicked() {
                select.write(Select(vec![entity]));
            }

            if let Some(drag) =
                BeatRangeDragZone::new(outer, ("noise-visible-range", entity), &ctx, &mut *area)
                    .show(ui)
            {
                edit.write(DoCommand(EditorCommand::EditNoiseArea(EditNoiseArea {
                    entity,
                    from: drag.from,
                    to: drag.to,
                })));
            }

            let mut active = ActiveRange {
                start: area.enable_beat,
                end: area.disable_beat,
            };
            if let Some(drag) =
                BeatRangeDragZone::new(inner, ("noise-active-range", entity), &ctx, &mut active)
                    .show(ui)
            {
                let mut from = area.clone();
                from.enable_beat = drag.from.start;
                from.disable_beat = drag.from.end;
                area.enable_beat = drag.to.start.max(area.appear_beat);
                area.disable_beat = drag.to.end.min(area.disappear_beat);
                edit.write(DoCommand(EditorCommand::EditNoiseArea(EditNoiseArea {
                    entity,
                    from,
                    to: area.clone(),
                })));
            }

            edit_keyframes(ui, &ctx, x, column_width, entity, &mut area, &mut edit);
        }
    }

    fn on_drag_selection(&self, world: &mut World, viewport: Rect, selection: Rect) -> Vec<Entity> {
        let mut state: SystemState<(Query<(Entity, &NoiseArea, &NoiseAreaOrder)>, Res<BpmList>)> =
            SystemState::new(world);
        let (areas, bpm_list) = state.get_mut(world);
        let count = areas.iter().count().max(1);
        let columns = count.min(8);
        let width = viewport.width() / columns as f32;

        areas
            .iter()
            .filter(|(_, area, order)| {
                let x = width * (order.0 % columns) as f32 + width / 2.0;
                selection.x_range().contains(x)
                    && selection
                        .y_range()
                        .contains(bpm_list.time_at(area.enable_beat))
            })
            .map(|(entity, _, _)| entity)
            .collect()
    }

    fn name(&self, _world: &World) -> String {
        format!(
            "{} {}",
            egui_phosphor::regular::WAVE_SINE,
            t!("tab.noise_areas.title")
        )
    }
}

fn beat_rect(ctx: &TimelineContext, left: f32, width: f32, start: Beat, end: Beat) -> Rect {
    let bottom = ctx.beat_to_y(start);
    let top = ctx.beat_to_y(end);
    Rect::from_min_max(egui::pos2(left, top), egui::pos2(left + width, bottom))
}

#[derive(Clone, Copy)]
enum KeyKind {
    Move,
    Scale,
    Rotate,
}

fn edit_keyframes(
    ui: &mut Ui,
    ctx: &TimelineContext,
    x: f32,
    width: f32,
    entity: Entity,
    area: &mut NoiseArea,
    edits: &mut MessageWriter<DoCommand>,
) {
    let lanes = [
        (KeyKind::Move, 0.25, area.move_events.len()),
        (KeyKind::Scale, 0.5, area.scale_events.len()),
        (KeyKind::Rotate, 0.75, area.rotate_events.len()),
    ];
    for (kind, fraction, count) in lanes {
        for index in 0..count {
            let beat = match kind {
                KeyKind::Move => area.move_events[index].beat,
                KeyKind::Scale => area.scale_events[index].beat,
                KeyKind::Rotate => area.rotate_events[index].beat,
            };
            let center = egui::pos2(x + width * fraction, ctx.beat_to_y(beat));
            let hit_rect = Rect::from_center_size(center, egui::vec2(12.0, 12.0));
            let id = egui::Id::new(("noise-keyframe", entity, kind as u8, index));
            let response = ui.interact(hit_rect, id, Sense::drag());
            let points = [
                center + egui::vec2(0.0, -4.0),
                center + egui::vec2(4.0, 0.0),
                center + egui::vec2(0.0, 4.0),
                center + egui::vec2(-4.0, 0.0),
            ];
            ui.painter().add(egui::Shape::convex_polygon(
                points.to_vec(),
                Color32::WHITE,
                Stroke::NONE,
            ));
            if response.drag_started() {
                ui.data_mut(|data| data.insert_temp(id.with("snapshot"), area.clone()));
                ui.data_mut(|data| data.insert_temp(id.with("y"), center.y));
            }
            if response.dragged() {
                let Some(initial_y) = ui.data(|data| data.get_temp::<f32>(id.with("y"))) else {
                    continue;
                };
                let new_beat = ctx.settings.attach(
                    ctx.y_to_beat_f32(initial_y + response.drag_delta().y)
                        .max(0.0),
                );
                match kind {
                    KeyKind::Move => area.move_events[index].beat = new_beat,
                    KeyKind::Scale => area.scale_events[index].beat = new_beat,
                    KeyKind::Rotate => area.rotate_events[index].beat = new_beat,
                }
            }
            if response.drag_stopped() {
                let from = ui.data(|data| data.get_temp::<NoiseArea>(id.with("snapshot")));
                ui.data_mut(|data| {
                    data.remove::<NoiseArea>(id.with("snapshot"));
                    data.remove::<f32>(id.with("y"));
                });
                area.move_events.sort_by_key(|event| event.beat);
                area.scale_events.sort_by_key(|event| event.beat);
                area.rotate_events.sort_by_key(|event| event.beat);
                if let Some(from) = from {
                    if from != *area {
                        edits.write(DoCommand(EditorCommand::EditNoiseArea(EditNoiseArea {
                            entity,
                            from,
                            to: area.clone(),
                        })));
                    }
                }
            }
        }
    }
}
