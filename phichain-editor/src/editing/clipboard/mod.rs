use crate::action::ActionRegistrationExt;
use crate::editing::command::event::{CreateEvent, RemoveEvent};
use crate::editing::command::noise::{CreateNoiseArea, RemoveNoiseArea};
use crate::editing::command::note::{CreateNote, RemoveNote};
use crate::editing::command::{CommandSequence, EditorCommand};
use crate::editing::DoCommand;
use crate::hotkey::modifier::Modifier;
use crate::hotkey::Hotkey;
use crate::selection::{Selected, SelectedLine};
use crate::timeline::TimelineContext;
use crate::timing::ChartTime;
use crate::utils::convert::BevyEguiConvert;
use bevy::prelude::*;
use phichain_chart::bpm_list::BpmList;
use phichain_chart::event::LineEvent;
use phichain_chart::noise::NoiseArea;
use phichain_chart::note::Note;

#[derive(Resource, Default)]
struct EditorClipboard {
    notes: Vec<Note>,
    events: Vec<LineEvent>,
    noise_areas: Vec<NoiseArea>,
}

impl EditorClipboard {
    fn clear(&mut self) {
        self.notes.clear();
        self.events.clear();
        self.noise_areas.clear();
    }
}

pub struct ClipboardPlugin;

impl Plugin for ClipboardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorClipboard>()
            .add_action(
                "phichain.copy",
                copy_system,
                Some(Hotkey::new(KeyCode::KeyC, vec![Modifier::Control])),
            )
            .add_action(
                "phichain.cut",
                cut_system,
                Some(Hotkey::new(KeyCode::KeyX, vec![Modifier::Control])),
            )
            .add_action(
                "phichain.paste",
                paste_system,
                Some(Hotkey::new(KeyCode::KeyV, vec![Modifier::Control])),
            );
    }
}

fn copy_system(
    mut clipboard: ResMut<EditorClipboard>,

    note_query: Query<&Note>,
    event_query: Query<&LineEvent>,
    noise_query: Query<&NoiseArea>,

    selected_query: Query<Entity, With<Selected>>,
) -> Result {
    clipboard.clear();

    for entity in &selected_query {
        if let Ok(note) = note_query.get(entity) {
            clipboard.notes.push(*note);
        } else if let Ok(event) = event_query.get(entity) {
            clipboard.events.push(*event);
        } else if let Ok(area) = noise_query.get(entity) {
            clipboard.noise_areas.push(area.clone());
        }
    }

    Ok(())
}

fn cut_system(
    mut clipboard: ResMut<EditorClipboard>,

    note_query: Query<&Note>,
    event_query: Query<&LineEvent>,
    noise_query: Query<&NoiseArea>,

    selected_query: Query<Entity, With<Selected>>,

    mut event_writer: MessageWriter<DoCommand>,
) -> Result {
    clipboard.clear();

    let mut commands = vec![];

    for entity in &selected_query {
        if let Ok(note) = note_query.get(entity) {
            clipboard.notes.push(*note);
            commands.push(EditorCommand::RemoveNote(RemoveNote::new(entity)));
        } else if let Ok(event) = event_query.get(entity) {
            clipboard.events.push(*event);
            commands.push(EditorCommand::RemoveEvent(RemoveEvent::new(entity)));
        } else if let Ok(area) = noise_query.get(entity) {
            clipboard.noise_areas.push(area.clone());
            commands.push(EditorCommand::RemoveNoiseArea(RemoveNoiseArea::new(entity)));
        }
    }

    event_writer.write(DoCommand(EditorCommand::CommandSequence(CommandSequence(
        commands,
    ))));

    Ok(())
}

fn paste_system(
    clipboard: Res<EditorClipboard>,

    window_query: Query<&Window>,

    selected_line: Res<SelectedLine>,

    ctx: TimelineContext,
    bpm_list: Res<BpmList>,
    chart_time: Res<ChartTime>,

    mut event_writer: MessageWriter<DoCommand>,
) -> Result {
    let notes = clipboard.notes.to_vec();
    let events = clipboard.events.to_vec();
    let noise_areas = clipboard.noise_areas.to_vec();
    let mut sequence = CommandSequence(vec![]);

    if let Some(min_beat) = noise_areas.iter().map(|area| area.appear_beat).min() {
        let target = bpm_list.beat_at(chart_time.0);
        let delta = target - min_beat;
        for mut area in noise_areas {
            area.appear_beat += delta;
            area.enable_beat += delta;
            area.disable_beat += delta;
            area.disappear_beat += delta;
            for event in &mut area.move_events {
                event.beat += delta;
            }
            for event in &mut area.scale_events {
                event.beat += delta;
            }
            for event in &mut area.rotate_events {
                event.beat += delta;
            }
            sequence
                .0
                .push(EditorCommand::CreateNoiseArea(CreateNoiseArea::new(area)));
        }
    }

    if let Ok(window) = window_query.single() {
        if let Some(cursor_position) = window.cursor_position() {
            if ctx.viewport.0.contains(cursor_position) {
                let timeline = ctx
                    .settings
                    .container
                    .allocate(ctx.viewport.0.into_egui())
                    .iter()
                    .find(|x| x.viewport.x_range().contains(cursor_position.x))
                    .map(|x| x.timeline);

                if let Some(timeline) = timeline {
                    let target_line = timeline.line_entity().unwrap_or(selected_line.0);
                    if let Some(min_beat) = notes
                        .iter()
                        .map(|note| note.beat)
                        .chain(events.iter().map(|event| event.start_beat))
                        .min()
                    {
                        let time = ctx.y_to_time(cursor_position.y);
                        let beat = ctx.settings.attach(bpm_list.beat_at(time).value());
                        let delta = beat - min_beat;

                        for note in notes {
                            let mut new_note = note;
                            new_note.beat = note.beat + delta;
                            sequence.0.push(EditorCommand::CreateNote(CreateNote::new(
                                target_line,
                                new_note,
                            )));
                        }
                        for event in events {
                            let mut new_event = event;
                            new_event.start_beat = event.start_beat + delta;
                            new_event.end_beat = event.end_beat + delta;
                            sequence.0.push(EditorCommand::CreateEvent(CreateEvent::new(
                                target_line,
                                new_event,
                            )));
                        }
                    }
                }
            }
        }
    }

    if !sequence.0.is_empty() {
        event_writer.write(DoCommand(EditorCommand::CommandSequence(sequence)));
    }

    Ok(())
}
