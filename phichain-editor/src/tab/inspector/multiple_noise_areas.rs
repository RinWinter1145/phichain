use crate::editing::command::noise::EditNoiseArea;
use crate::editing::command::{CommandSequence, EditorCommand};
use crate::editing::DoCommand;
use crate::selection::Selected;
use bevy::prelude::*;
use egui::Ui;
use phichain_chart::beat::Beat;
use phichain_chart::noise::NoiseArea;

#[derive(Clone, Copy)]
enum BatchEdit {
    Normal,
    Subtract,
    Shift(Beat),
}

pub fn multiple_noise_areas_inspector(
    In(mut ui): In<Ui>,
    areas: Query<(Entity, &NoiseArea), With<Selected>>,
    mut edits: MessageWriter<DoCommand>,
) -> Result {
    let selected = areas
        .iter()
        .map(|(entity, area)| (entity, area.clone()))
        .collect::<Vec<_>>();
    ui.heading(t!(
        "tab.inspector.multiple_noise_areas.title",
        amount = selected.len()
    ));
    ui.label(t!("tab.inspector.multiple_noise_areas.hint"));
    ui.separator();

    let mut operation = None;
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(t!("tab.inspector.multiple_noise_areas.normal"))
            .clicked()
        {
            operation = Some(BatchEdit::Normal);
        }
        if ui
            .button(t!("tab.inspector.multiple_noise_areas.subtract"))
            .clicked()
        {
            operation = Some(BatchEdit::Subtract);
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(t!("tab.inspector.multiple_noise_areas.earlier"))
            .clicked()
        {
            let earliest = selected
                .iter()
                .map(|(_, area)| area.appear_beat)
                .min()
                .unwrap_or(Beat::ZERO);
            operation = Some(BatchEdit::Shift(if earliest < Beat::ONE {
                Beat::ZERO - earliest
            } else {
                Beat::ZERO - Beat::ONE
            }));
        }
        if ui
            .button(t!("tab.inspector.multiple_noise_areas.later"))
            .clicked()
        {
            operation = Some(BatchEdit::Shift(Beat::ONE));
        }
    });

    if let Some(operation) = operation {
        let commands = selected
            .into_iter()
            .filter_map(|(entity, from)| {
                let mut to = from.clone();
                match operation {
                    BatchEdit::Normal => to.is_subtract = false,
                    BatchEdit::Subtract => to.is_subtract = true,
                    BatchEdit::Shift(delta) => {
                        to.appear_beat += delta;
                        to.enable_beat += delta;
                        to.disable_beat += delta;
                        to.disappear_beat += delta;
                        for event in &mut to.move_events {
                            event.beat += delta;
                        }
                        for event in &mut to.scale_events {
                            event.beat += delta;
                        }
                        for event in &mut to.rotate_events {
                            event.beat += delta;
                        }
                    }
                }
                (from != to).then_some(EditorCommand::EditNoiseArea(EditNoiseArea {
                    entity,
                    from,
                    to,
                }))
            })
            .collect::<Vec<_>>();
        if !commands.is_empty() {
            edits.write(DoCommand(EditorCommand::CommandSequence(CommandSequence(
                commands,
            ))));
        }
    }
    Ok(())
}
