use crate::official::from_phichain::phichain_to_official;
use crate::official::into_phichain::official_to_phichain;
use crate::{ChartFormat, CommonOutputOptions};
use phichain_chart::event::LineEvent;
use phichain_chart::serialization::PhichainChart;
use phichain_compiler::helpers::are_contiguous;

mod errors;
mod from_phichain;
mod into_phichain;
mod options;
mod schema;

pub use errors::{OfficialInputError, OfficialOutputError};
pub use options::{OfficialInputOptions, OfficialOutputOptions};
pub use schema::OfficialChart;

const DEFAULT_EASING_FITTING_EPSILON: f32 = 1e-1;

fn merge_constant_events(events: Vec<LineEvent>) -> Vec<LineEvent> {
    events.into_iter().fold(Vec::new(), |mut merged, event| {
        if let Some(last) = merged.last_mut() {
            if last.value.is_numeric_constant()
                && event.value.is_numeric_constant()
                && are_contiguous(last, &event)
            {
                // extend the previous event instead of adding a new one
                last.end_beat = event.end_beat;
                return merged;
            }
        }
        merged.push(event);
        merged
    })
}

impl ChartFormat for OfficialChart {
    type InputOptions = OfficialInputOptions;
    type InputError = OfficialInputError;

    type OutputOptions = OfficialOutputOptions;
    type OutputError = OfficialOutputError;

    fn to_phichain(self, opts: &Self::InputOptions) -> Result<PhichainChart, Self::InputError> {
        official_to_phichain(self, opts)
    }

    fn from_phichain(
        phichain: PhichainChart,
        opts: &Self::OutputOptions,
    ) -> Result<Self, Self::OutputError> {
        phichain_to_official(phichain, opts)
    }

    fn apply_common_output_options(mut self, common_options: &CommonOutputOptions) -> Self {
        let round = |value: f32| -> f32 {
            let multiplier = 10_f32.powi(common_options.round as i32);
            (value * multiplier).round() / multiplier
        };

        for line in &mut self.lines {
            for event in &mut line.rotate_events {
                event.start = round(event.start);
                event.end = round(event.end);
            }

            for event in &mut line.opacity_events {
                event.start = round(event.start);
                event.end = round(event.end);
            }

            for event in &mut line.speed_events {
                event.value = round(event.value);
            }

            for event in &mut line.move_events {
                event.start_x = round(event.start_x);
                event.end_x = round(event.end_x);
                event.start_y = round(event.start_y);
                event.end_y = round(event.end_y);
            }

            for note in &mut line.notes_above {
                note.x = round(note.x);
            }
            for note in &mut line.notes_below {
                note.x = round(note.x);
            }
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phichain_chart::beat;

    #[test]
    fn block_area_round_trips_with_official_field_names() {
        let json = r#"{
          "formatVersion": 3,
          "offset": 0.0,
          "judgeLineList": [{
            "bpm": 120.0,
            "judgeLineMoveEvents": [], "judgeLineRotateEvents": [],
            "judgeLineDisappearEvents": [], "speedEvents": [],
            "notesAbove": [], "notesBelow": []
          }],
          "blockAreaList": [{
            "topRightPercentage": {"x": 0.9, "y": 0.8},
            "bottomLeftPercentage": {"x": 0.1, "y": 0.2},
            "appearTime": 1.0, "enableTime": 1.5,
            "disableTime": 3.0, "disappearTime": 3.5,
            "isSubtract": true,
            "moveEvents": [{"time": 2.0, "endPosition": {"x": 0.6, "y": 0.4}, "easeTypeX": 3, "easeTypeY": 4}],
            "scaleEvents": [{"time": 2.25, "anchor": {"x": 0.5, "y": 0.5}, "scale": {"x": 1.2, "y": 0.8}, "easeTypeX": 5, "easeTypeY": 6}],
            "rotateEvents": [{"time": 2.5, "anchor": {"x": 0.5, "y": 0.5}, "rotation": 45.0, "easeType": 7}]
          }]
        }"#;
        let official: OfficialChart = serde_json::from_str(json).unwrap();
        let phichain = official
            .to_phichain(&OfficialInputOptions::default())
            .unwrap();
        let area = &phichain.noise_areas.0[0];
        assert!(area.is_subtract);
        assert_eq!(area.appear_beat, beat!(2));
        assert_eq!(area.enable_beat, beat!(3));
        assert_eq!(area.move_events[0].beat, beat!(4));
        assert!((area.top_right_percentage.y - 0.2).abs() < 1e-5);
        assert!((area.bottom_left_percentage.y - 0.8).abs() < 1e-5);
        assert!((area.move_events[0].end_position.y - 0.6).abs() < 1e-5);
        assert_eq!(area.move_events[0].ease_type_x, 3);
        assert!((area.scale_events[0].scale.y - 0.8).abs() < 1e-5);
        assert_eq!(area.scale_events[0].ease_type_y, 6);
        assert_eq!(area.rotate_events[0].rotation, -45.0);

        let output =
            OfficialChart::from_phichain(phichain, &OfficialOutputOptions::default()).unwrap();
        let value = serde_json::to_value(output).unwrap();
        let area = &value["blockAreaList"][0];
        assert_eq!(area["isSubtract"], true);
        assert!((area["moveEvents"][0]["endPosition"]["x"].as_f64().unwrap() - 0.6).abs() < 1e-5);
        assert_eq!(area["scaleEvents"][0]["easeTypeY"], 6);
        assert_eq!(area["rotateEvents"][0]["easeType"], 7);
        assert_eq!(area["rotateEvents"][0]["rotation"], 45.0);
    }
}
