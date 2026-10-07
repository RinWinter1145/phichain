use crate::editing::command::noise::EditNoiseArea;
use crate::editing::command::EditorCommand;
use crate::editing::DoCommand;
use crate::selection::Selected;
use crate::timing::ChartTime;
use crate::ui::latch;
use crate::ui::sides::SidesExt;
use crate::ui::widgets::beat_value::BeatValue;
use bevy::prelude::*;
use egui::{DragValue, Ui};
use phichain_chart::beat::Beat;
use phichain_chart::bpm_list::BpmList;
use phichain_chart::noise::{
    NoiseArea, NoiseMoveEvent, NoisePoint, NoiseRotateEvent, NoiseScaleEvent,
};

pub fn noise_area_inspector(
    In(mut ui): In<Ui>,
    area: Single<(&mut NoiseArea, Entity), With<Selected>>,
    bpm_list: Res<BpmList>,
    chart_time: Res<ChartTime>,
    mut edits: MessageWriter<DoCommand>,
) -> Result {
    let (mut area, entity) = area.into_inner();
    let current_beat = bpm_list.beat_at(chart_time.0);

    ui.heading(t!("tab.inspector.noise_area.title"));
    ui.label(t!("tab.inspector.noise_area.hint"));
    ui.separator();

    let previous = latch::latch(&mut ui, "selected_noise_area", area.clone(), |ui| {
        let mut finished = false;
        ui.sides(
            |ui| ui.label(t!("tab.noise_areas.is_subtract")),
            |ui| finished |= ui.checkbox(&mut area.is_subtract, "").changed(),
        );
        let mut visual_only = area.is_visual_only();
        ui.sides(
            |ui| ui.label(t!("tab.noise_areas.visual_only")),
            |ui| {
                if ui.checkbox(&mut visual_only, "").changed() {
                    area.set_visual_only(visual_only);
                    finished = true;
                }
            },
        );

        let appear = area.appear_beat;
        let enable = area.enable_beat;
        let disable = area.disable_beat;
        let disappear = area.disappear_beat;
        finished |= beat_row(
            ui,
            t!("tab.noise_areas.appear"),
            &mut area.appear_beat,
            Beat::MIN,
            enable,
        );
        finished |= beat_row(
            ui,
            t!("tab.noise_areas.enable"),
            &mut area.enable_beat,
            appear,
            disable,
        );
        finished |= beat_row(
            ui,
            t!("tab.noise_areas.disable"),
            &mut area.disable_beat,
            enable,
            disappear,
        );
        finished |= beat_row(
            ui,
            t!("tab.noise_areas.disappear"),
            &mut area.disappear_beat,
            disable,
            Beat::MAX,
        );

        ui.separator();
        ui.label(t!("tab.inspector.noise_area.presets"));
        ui.horizontal_wrapped(|ui| {
            finished |= preset(
                ui,
                t!("tab.inspector.noise_area.full"),
                &mut area,
                [0.0, 0.0, 1.0, 1.0],
            );
            finished |= preset(
                ui,
                t!("tab.inspector.noise_area.left"),
                &mut area,
                [0.0, 0.0, 0.5, 1.0],
            );
            finished |= preset(
                ui,
                t!("tab.inspector.noise_area.right"),
                &mut area,
                [0.5, 0.0, 1.0, 1.0],
            );
            finished |= preset(
                ui,
                t!("tab.inspector.noise_area.top"),
                &mut area,
                [0.0, 0.5, 1.0, 1.0],
            );
            finished |= preset(
                ui,
                t!("tab.inspector.noise_area.bottom"),
                &mut area,
                [0.0, 0.0, 1.0, 0.5],
            );
        });

        ui.collapsing(t!("tab.inspector.noise_area.geometry"), |ui| {
            finished |= point_row(
                ui,
                t!("tab.noise_areas.bottom_left"),
                &mut area.bottom_left_percentage,
            );
            finished |= point_row(
                ui,
                t!("tab.noise_areas.top_right"),
                &mut area.top_right_percentage,
            );
        });

        ui.separator();
        ui.label(t!(
            "tab.inspector.noise_area.keyframes",
            beat = current_beat.value()
        ));
        ui.horizontal_wrapped(|ui| {
            if ui.button(t!("tab.inspector.noise_area.add_move")).clicked() {
                upsert_move(&mut area, current_beat, chart_time.0, &bpm_list);
                finished = true;
            }
            if ui
                .button(t!("tab.inspector.noise_area.add_scale"))
                .clicked()
            {
                upsert_scale(&mut area, current_beat, chart_time.0, &bpm_list);
                finished = true;
            }
            if ui
                .button(t!("tab.inspector.noise_area.add_rotate"))
                .clicked()
            {
                upsert_rotate(&mut area, current_beat, chart_time.0, &bpm_list);
                finished = true;
            }
        });

        finished |= current_keyframes(ui, &mut area, current_beat);
        finished
    });

    if let Some(from) = previous {
        if from != *area {
            edits.write(DoCommand(EditorCommand::EditNoiseArea(EditNoiseArea {
                entity,
                from,
                to: area.clone(),
            })));
        }
    }
    Ok(())
}

fn done(response: egui::Response) -> bool {
    response.changed() && (response.drag_stopped() || response.lost_focus() || response.clicked())
}

fn beat_row(
    ui: &mut Ui,
    label: impl Into<egui::WidgetText>,
    beat: &mut Beat,
    min: Beat,
    max: Beat,
) -> bool {
    let mut result = false;
    ui.sides(
        |ui| ui.label(label),
        |ui| result = done(ui.add(BeatValue::new(beat).range(min..=max))),
    );
    result
}

fn point_row(ui: &mut Ui, label: impl Into<egui::WidgetText>, point: &mut NoisePoint) -> bool {
    let mut result = false;
    ui.sides(
        |ui| ui.label(label),
        |ui| {
            ui.horizontal(|ui| {
                result |= done(ui.add(DragValue::new(&mut point.x).speed(0.001)));
                result |= done(ui.add(DragValue::new(&mut point.y).speed(0.001)));
            });
        },
    );
    result
}

fn preset(
    ui: &mut Ui,
    label: impl Into<egui::WidgetText>,
    area: &mut NoiseArea,
    rect: [f32; 4],
) -> bool {
    if ui.small_button(label).clicked() {
        area.bottom_left_percentage = NoisePoint::new(rect[0], rect[1]);
        area.top_right_percentage = NoisePoint::new(rect[2], rect[3]);
        true
    } else {
        false
    }
}

fn upsert_move(area: &mut NoiseArea, beat: Beat, seconds: f32, bpm_list: &BpmList) {
    if !area.move_events.iter().any(|event| event.beat == beat) {
        let visual_center = area.rect_at(seconds, bpm_list).center;
        area.set_move_key(NoiseMoveEvent {
            beat,
            end_position: area.move_target_for_center(seconds, bpm_list, visual_center),
            ease_type_x: 0,
            ease_type_y: 0,
        });
    }
}

fn upsert_scale(area: &mut NoiseArea, beat: Beat, seconds: f32, bpm_list: &BpmList) {
    if !area.scale_events.iter().any(|event| event.beat == beat) {
        let visual = area.rect_at(seconds, bpm_list);
        let base_size = Vec2::new(
            (area.top_right_percentage.x - area.bottom_left_percentage.x).abs()
                * phichain_chart::constants::CANVAS_WIDTH,
            (area.top_right_percentage.y - area.bottom_left_percentage.y).abs()
                * phichain_chart::constants::CANVAS_HEIGHT,
        );
        area.set_scale_key(NoiseScaleEvent {
            beat,
            anchor: NoisePoint::from_world(visual.center),
            scale: NoisePoint::new(
                visual.size.x / base_size.x.max(1.0),
                visual.size.y / base_size.y.max(1.0),
            ),
            ease_type_x: 0,
            ease_type_y: 0,
        });
        preserve_visual_center(area, beat, seconds, bpm_list, visual.center);
    }
}

fn upsert_rotate(area: &mut NoiseArea, beat: Beat, seconds: f32, bpm_list: &BpmList) {
    if !area.rotate_events.iter().any(|event| event.beat == beat) {
        let visual = area.rect_at(seconds, bpm_list);
        area.set_rotate_key(NoiseRotateEvent {
            beat,
            anchor: NoisePoint::from_world(visual.center),
            rotation: visual.rotation_degrees,
            ease_type: 0,
        });
        preserve_visual_center(area, beat, seconds, bpm_list, visual.center);
    }
}

fn preserve_visual_center(
    area: &mut NoiseArea,
    beat: Beat,
    seconds: f32,
    bpm_list: &BpmList,
    center: Vec2,
) {
    let actual = area.rect_at(seconds, bpm_list).center;
    if (actual - center).length_squared() <= 0.0001 {
        return;
    }
    let (ease_type_x, ease_type_y) = area
        .move_events
        .iter()
        .find(|event| event.beat == beat)
        .map(|event| (event.ease_type_x, event.ease_type_y))
        .unwrap_or((0, 0));
    area.set_move_key(NoiseMoveEvent {
        beat,
        end_position: area.move_target_for_center(seconds, bpm_list, center),
        ease_type_x,
        ease_type_y,
    });
}

fn current_keyframes(ui: &mut Ui, area: &mut NoiseArea, beat: Beat) -> bool {
    let mut finished = false;
    let mut remove_move = None;
    if let Some(index) = area.move_events.iter().position(|event| event.beat == beat) {
        let event = &mut area.move_events[index];
        ui.collapsing(t!("tab.noise_areas.move_events"), |ui| {
            finished |= point_row(
                ui,
                t!("tab.noise_areas.end_position"),
                &mut event.end_position,
            );
            finished |= easing_row(ui, &mut event.ease_type_x, &mut event.ease_type_y);
            if ui.button(t!("tab.noise_areas.remove_event")).clicked() {
                remove_move = Some(index);
            }
        });
    }
    if let Some(index) = remove_move {
        area.move_events.remove(index);
        finished = true;
    }

    let mut remove_scale = None;
    if let Some(index) = area
        .scale_events
        .iter()
        .position(|event| event.beat == beat)
    {
        let event = &mut area.scale_events[index];
        ui.collapsing(t!("tab.noise_areas.scale_events"), |ui| {
            finished |= point_row(ui, t!("tab.noise_areas.anchor"), &mut event.anchor);
            finished |= point_row(ui, t!("tab.noise_areas.scale"), &mut event.scale);
            finished |= easing_row(ui, &mut event.ease_type_x, &mut event.ease_type_y);
            if ui.button(t!("tab.noise_areas.remove_event")).clicked() {
                remove_scale = Some(index);
            }
        });
    }
    if let Some(index) = remove_scale {
        area.scale_events.remove(index);
        finished = true;
    }

    let mut remove_rotate = None;
    if let Some(index) = area
        .rotate_events
        .iter()
        .position(|event| event.beat == beat)
    {
        let event = &mut area.rotate_events[index];
        ui.collapsing(t!("tab.noise_areas.rotate_events"), |ui| {
            finished |= point_row(ui, t!("tab.noise_areas.anchor"), &mut event.anchor);
            ui.sides(
                |ui| ui.label(t!("tab.noise_areas.rotation")),
                |ui| {
                    finished |= done(ui.add(DragValue::new(&mut event.rotation).speed(0.5)));
                },
            );
            ui.sides(
                |ui| ui.label("Easing"),
                |ui| {
                    finished |= easing_picker(ui, &mut event.ease_type);
                },
            );
            if ui.button(t!("tab.noise_areas.remove_event")).clicked() {
                remove_rotate = Some(index);
            }
        });
    }
    if let Some(index) = remove_rotate {
        area.rotate_events.remove(index);
        finished = true;
    }
    finished
}

fn easing_row(ui: &mut Ui, x: &mut u8, y: &mut u8) -> bool {
    let mut finished = false;
    ui.sides(
        |ui| ui.label("Easing X/Y"),
        |ui| {
            ui.horizontal(|ui| {
                ui.label("X");
                finished |= easing_picker(ui, x);
                ui.label("Y");
                finished |= easing_picker(ui, y);
            });
        },
    );
    finished
}

fn easing_picker(ui: &mut Ui, value: &mut u8) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(ui.next_auto_id())
        .selected_text(easing_name(*value))
        .width(92.0)
        .show_ui(ui, |ui| {
            for candidate in 0..=14 {
                changed |= ui
                    .selectable_value(value, candidate, easing_name(candidate))
                    .changed();
            }
        });
    changed
}

fn easing_name(value: u8) -> &'static str {
    match value {
        0 => "0 · Linear",
        1 => "1 · Quad In",
        2 => "2 · Quad Out",
        3 => "3 · Quad In/Out",
        4 => "4 · Cubic In",
        5 => "5 · Cubic Out",
        6 => "6 · Cubic In/Out",
        7 => "7 · Quart In",
        8 => "8 · Quart Out",
        9 => "9 · Quart In/Out",
        10 => "10 · Quint In",
        11 => "11 · Quint Out",
        12 => "12 · Quint In/Out",
        13 => "13 · Hold",
        14 => "14 · Instant",
        _ => "Unknown",
    }
}
