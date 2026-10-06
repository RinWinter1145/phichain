use crate::editing::command::noise::{CreateNoiseArea, EditNoiseArea, RemoveNoiseArea};
use crate::editing::command::EditorCommand;
use crate::editing::DoCommand;
use crate::ui::latch;
use crate::ui::widgets::beat_value::BeatValue;
use bevy::prelude::*;
use egui::{ScrollArea, Ui};
use phichain_chart::beat::Beat;
use phichain_chart::noise::{
    NoiseArea, NoiseMoveEvent, NoisePoint, NoiseRotateEvent, NoiseScaleEvent,
};
use phichain_game::noise::NoiseAreaOrder;

pub fn noise_areas_tab(
    In(mut ui): In<Ui>,
    mut areas: Query<(Entity, &mut NoiseArea, &NoiseAreaOrder)>,
    mut commands: MessageWriter<DoCommand>,
) {
    let mut remove = None;

    ScrollArea::vertical().show(&mut ui, |ui| {
        let mut sorted = areas.iter_mut().collect::<Vec<_>>();
        sorted.sort_by_key(|(_, _, order)| order.0);
        for (entity, mut area, order) in sorted {
            let index = order.0;
            ui.push_id(entity, |ui| {
                let title = if area.is_subtract {
                    format!("#{} {}", index + 1, t!("tab.noise_areas.subtract"))
                } else {
                    format!("#{} {}", index + 1, t!("tab.noise_areas.normal"))
                };

                ui.collapsing(title, |ui| {
                    let previous = latch::latch(ui, "noise_area", area.clone(), |ui| {
                        let mut finished = false;
                        let appear_beat = area.appear_beat;
                        let enable_beat = area.enable_beat;
                        let disable_beat = area.disable_beat;
                        let disappear_beat = area.disappear_beat;
                        egui::Grid::new("base")
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                finished |= checkbox_row(
                                    ui,
                                    t!("tab.noise_areas.is_subtract"),
                                    &mut area.is_subtract,
                                );
                                finished |= point_row(
                                    ui,
                                    t!("tab.noise_areas.top_right"),
                                    &mut area.top_right_percentage,
                                );
                                finished |= point_row(
                                    ui,
                                    t!("tab.noise_areas.bottom_left"),
                                    &mut area.bottom_left_percentage,
                                );
                                finished |= beat_row(
                                    ui,
                                    t!("tab.noise_areas.appear"),
                                    &mut area.appear_beat,
                                    Beat::MIN,
                                    enable_beat,
                                );
                                finished |= beat_row(
                                    ui,
                                    t!("tab.noise_areas.enable"),
                                    &mut area.enable_beat,
                                    appear_beat,
                                    disable_beat,
                                );
                                finished |= beat_row(
                                    ui,
                                    t!("tab.noise_areas.disable"),
                                    &mut area.disable_beat,
                                    enable_beat,
                                    disappear_beat,
                                );
                                finished |= beat_row(
                                    ui,
                                    t!("tab.noise_areas.disappear"),
                                    &mut area.disappear_beat,
                                    disable_beat,
                                    Beat::MAX,
                                );
                            });

                        ui.separator();
                        finished |= move_events_ui(ui, &mut area.move_events);
                        finished |= scale_events_ui(ui, &mut area.scale_events);
                        finished |= rotate_events_ui(ui, &mut area.rotate_events);
                        finished
                    });

                    if let Some(from) = previous {
                        if from != *area {
                            commands.write(DoCommand(EditorCommand::EditNoiseArea(
                                EditNoiseArea {
                                    entity,
                                    from,
                                    to: area.clone(),
                                },
                            )));
                        }
                    }

                    if ui.button(t!("tab.noise_areas.remove")).clicked() {
                        remove = Some(entity);
                    }
                });
                ui.separator();
            });
        }

        if ui.button(t!("tab.noise_areas.new")).clicked() {
            commands.write(DoCommand(EditorCommand::CreateNoiseArea(
                CreateNoiseArea::new(NoiseArea::default()),
            )));
        }
    });

    if let Some(entity) = remove {
        commands.write(DoCommand(EditorCommand::RemoveNoiseArea(
            RemoveNoiseArea::new(entity),
        )));
    }
}

fn done(response: egui::Response) -> bool {
    response.changed() && (response.drag_stopped() || response.lost_focus() || response.clicked())
}

fn checkbox_row(ui: &mut Ui, label: impl Into<egui::WidgetText>, value: &mut bool) -> bool {
    ui.label(label);
    let response = ui.checkbox(value, "");
    ui.end_row();
    done(response)
}

fn point_row(ui: &mut Ui, label: impl Into<egui::WidgetText>, point: &mut NoisePoint) -> bool {
    ui.label(label);
    let mut finished = false;
    ui.horizontal(|ui| {
        ui.label("x");
        finished |= done(ui.add(egui::DragValue::new(&mut point.x).speed(0.001)));
        ui.label("y");
        finished |= done(ui.add(egui::DragValue::new(&mut point.y).speed(0.001)));
    });
    ui.end_row();
    finished
}

fn beat_row(
    ui: &mut Ui,
    label: impl Into<egui::WidgetText>,
    beat: &mut Beat,
    min: Beat,
    max: Beat,
) -> bool {
    ui.label(label);
    let result = done(ui.add(BeatValue::new(beat).range(min..=max)));
    ui.end_row();
    result
}

fn ease_row(ui: &mut Ui, label: &str, ease: &mut u8) -> bool {
    ui.label(label);
    let result = done(ui.add(egui::DragValue::new(ease).range(0..=14)));
    ui.end_row();
    result
}

fn move_events_ui(ui: &mut Ui, events: &mut Vec<NoiseMoveEvent>) -> bool {
    let mut finished = false;
    ui.collapsing(
        format!("{} ({})", t!("tab.noise_areas.move_events"), events.len()),
        |ui| {
            let mut remove = None;
            for index in 0..events.len() {
                let min = index
                    .checked_sub(1)
                    .map(|i| events[i].beat)
                    .unwrap_or(Beat::MIN);
                let max = events.get(index + 1).map(|e| e.beat).unwrap_or(Beat::MAX);
                let event = &mut events[index];
                egui::Grid::new(format!("move_{index}"))
                    .num_columns(2)
                    .show(ui, |ui| {
                        finished |=
                            beat_row(ui, t!("tab.noise_areas.beat"), &mut event.beat, min, max);
                        finished |= point_row(
                            ui,
                            t!("tab.noise_areas.end_position"),
                            &mut event.end_position,
                        );
                        finished |= ease_row(ui, "Ease X", &mut event.ease_type_x);
                        finished |= ease_row(ui, "Ease Y", &mut event.ease_type_y);
                    });
                if ui
                    .small_button(t!("tab.noise_areas.remove_event"))
                    .clicked()
                {
                    remove = Some(index);
                }
                ui.separator();
            }
            if let Some(index) = remove {
                events.remove(index);
                finished = true;
            }
            if ui.button(t!("tab.noise_areas.add_event")).clicked() {
                events.push(NoiseMoveEvent {
                    beat: events
                        .last()
                        .map(|e| e.beat + Beat::ONE)
                        .unwrap_or(Beat::ZERO),
                    end_position: NoisePoint::new(0.5, 0.5),
                    ease_type_x: 0,
                    ease_type_y: 0,
                });
                finished = true;
            }
        },
    );
    finished
}

fn scale_events_ui(ui: &mut Ui, events: &mut Vec<NoiseScaleEvent>) -> bool {
    let mut finished = false;
    ui.collapsing(
        format!("{} ({})", t!("tab.noise_areas.scale_events"), events.len()),
        |ui| {
            let mut remove = None;
            for index in 0..events.len() {
                let min = index
                    .checked_sub(1)
                    .map(|i| events[i].beat)
                    .unwrap_or(Beat::MIN);
                let max = events.get(index + 1).map(|e| e.beat).unwrap_or(Beat::MAX);
                let event = &mut events[index];
                egui::Grid::new(format!("scale_{index}"))
                    .num_columns(2)
                    .show(ui, |ui| {
                        finished |=
                            beat_row(ui, t!("tab.noise_areas.beat"), &mut event.beat, min, max);
                        finished |= point_row(ui, t!("tab.noise_areas.anchor"), &mut event.anchor);
                        finished |= point_row(ui, t!("tab.noise_areas.scale"), &mut event.scale);
                        finished |= ease_row(ui, "Ease X", &mut event.ease_type_x);
                        finished |= ease_row(ui, "Ease Y", &mut event.ease_type_y);
                    });
                if ui
                    .small_button(t!("tab.noise_areas.remove_event"))
                    .clicked()
                {
                    remove = Some(index);
                }
                ui.separator();
            }
            if let Some(index) = remove {
                events.remove(index);
                finished = true;
            }
            if ui.button(t!("tab.noise_areas.add_event")).clicked() {
                events.push(NoiseScaleEvent {
                    beat: events
                        .last()
                        .map(|e| e.beat + Beat::ONE)
                        .unwrap_or(Beat::ZERO),
                    anchor: NoisePoint::new(0.5, 0.5),
                    scale: NoisePoint::new(1.0, 1.0),
                    ease_type_x: 0,
                    ease_type_y: 0,
                });
                finished = true;
            }
        },
    );
    finished
}

fn rotate_events_ui(ui: &mut Ui, events: &mut Vec<NoiseRotateEvent>) -> bool {
    let mut finished = false;
    ui.collapsing(
        format!("{} ({})", t!("tab.noise_areas.rotate_events"), events.len()),
        |ui| {
            let mut remove = None;
            for index in 0..events.len() {
                let min = index
                    .checked_sub(1)
                    .map(|i| events[i].beat)
                    .unwrap_or(Beat::MIN);
                let max = events.get(index + 1).map(|e| e.beat).unwrap_or(Beat::MAX);
                let event = &mut events[index];
                egui::Grid::new(format!("rotate_{index}"))
                    .num_columns(2)
                    .show(ui, |ui| {
                        finished |=
                            beat_row(ui, t!("tab.noise_areas.beat"), &mut event.beat, min, max);
                        finished |= point_row(ui, t!("tab.noise_areas.anchor"), &mut event.anchor);
                        ui.label(t!("tab.noise_areas.rotation"));
                        finished |=
                            done(ui.add(egui::DragValue::new(&mut event.rotation).speed(0.1)));
                        ui.end_row();
                        finished |= ease_row(ui, "Ease", &mut event.ease_type);
                    });
                if ui
                    .small_button(t!("tab.noise_areas.remove_event"))
                    .clicked()
                {
                    remove = Some(index);
                }
                ui.separator();
            }
            if let Some(index) = remove {
                events.remove(index);
                finished = true;
            }
            if ui.button(t!("tab.noise_areas.add_event")).clicked() {
                events.push(NoiseRotateEvent {
                    beat: events
                        .last()
                        .map(|e| e.beat + Beat::ONE)
                        .unwrap_or(Beat::ZERO),
                    anchor: NoisePoint::new(0.5, 0.5),
                    rotation: 0.0,
                    ease_type: 0,
                });
                finished = true;
            }
        },
    );
    finished
}
