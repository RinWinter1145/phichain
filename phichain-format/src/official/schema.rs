//! Phigros official json chart format

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct OfficialPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OfficialBlockMoveEvent {
    pub time: f32,
    #[serde(rename = "endPosition")]
    pub end_position: OfficialPoint,
    #[serde(rename = "easeTypeX")]
    pub ease_type_x: u8,
    #[serde(rename = "easeTypeY")]
    pub ease_type_y: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OfficialBlockScaleEvent {
    pub time: f32,
    pub anchor: OfficialPoint,
    pub scale: OfficialPoint,
    #[serde(rename = "easeTypeX")]
    pub ease_type_x: u8,
    #[serde(rename = "easeTypeY")]
    pub ease_type_y: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OfficialBlockRotateEvent {
    pub time: f32,
    pub anchor: OfficialPoint,
    pub rotation: f32,
    #[serde(rename = "easeType")]
    pub ease_type: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OfficialBlockArea {
    #[serde(rename = "topRightPercentage")]
    pub top_right_percentage: OfficialPoint,
    #[serde(rename = "bottomLeftPercentage")]
    pub bottom_left_percentage: OfficialPoint,
    #[serde(rename = "appearTime")]
    pub appear_time: f32,
    #[serde(rename = "enableTime")]
    pub enable_time: f32,
    #[serde(rename = "disableTime")]
    pub disable_time: f32,
    #[serde(rename = "disappearTime")]
    pub disappear_time: f32,
    #[serde(rename = "isSubtract")]
    pub is_subtract: bool,
    #[serde(rename = "moveEvents", default)]
    pub move_events: Vec<OfficialBlockMoveEvent>,
    #[serde(rename = "scaleEvents", default)]
    pub scale_events: Vec<OfficialBlockScaleEvent>,
    #[serde(rename = "rotateEvents", default)]
    pub rotate_events: Vec<OfficialBlockRotateEvent>,
}

#[derive(Serialize_repr, Deserialize_repr, Debug)]
#[repr(u8)]
pub enum OfficialNoteKind {
    Tap = 1,
    Drag = 2,
    Hold = 3,
    Flick = 4,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OfficialNote {
    #[serde(rename = "type")]
    pub kind: OfficialNoteKind,
    pub time: f32,
    #[serde(rename = "holdTime")]
    pub hold_time: f32,
    #[serde(rename = "positionX")]
    pub x: f32,
    pub speed: f32,

    #[serde(rename = "floorPosition")]
    pub floor_position: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OfficialNumericLineEvent {
    #[serde(rename = "startTime")]
    pub start_time: f32,
    #[serde(rename = "endTime")]
    pub end_time: f32,
    pub start: f32,
    pub end: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OfficialPositionLineEvent {
    #[serde(rename = "startTime")]
    pub start_time: f32,
    #[serde(rename = "endTime")]
    pub end_time: f32,
    #[serde(rename = "start")]
    pub start_x: f32,
    // formatVersion 1 does not have start2
    #[serde(rename = "start2", default)]
    pub start_y: f32,
    #[serde(rename = "end")]
    pub end_x: f32,
    // formatVersion 1 does not have end2
    #[serde(rename = "end2", default)]
    pub end_y: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OfficialSpeedEvent {
    #[serde(rename = "startTime")]
    pub start_time: f32,
    #[serde(rename = "endTime")]
    pub end_time: f32,
    pub value: f32,

    #[serde(rename = "floorPosition", default)]
    pub floor_position: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OfficialLine {
    pub bpm: f32,

    #[serde(rename = "judgeLineMoveEvents")]
    pub move_events: Vec<OfficialPositionLineEvent>,
    #[serde(rename = "judgeLineRotateEvents")]
    pub rotate_events: Vec<OfficialNumericLineEvent>,
    #[serde(rename = "judgeLineDisappearEvents")]
    pub opacity_events: Vec<OfficialNumericLineEvent>,
    #[serde(rename = "speedEvents")]
    pub speed_events: Vec<OfficialSpeedEvent>,

    #[serde(rename = "notesAbove")]
    pub notes_above: Vec<OfficialNote>,
    #[serde(rename = "notesBelow")]
    pub notes_below: Vec<OfficialNote>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OfficialChart {
    #[serde(rename = "formatVersion")]
    pub format_version: u32,
    pub offset: f32,
    #[serde(rename = "judgeLineList")]
    pub lines: Vec<OfficialLine>,
    #[serde(rename = "blockAreaList", default)]
    pub block_areas: Vec<OfficialBlockArea>,
}
