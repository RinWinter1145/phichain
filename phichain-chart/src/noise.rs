use crate::beat::Beat;
use crate::bpm_list::BpmList;
use crate::constants::{CANVAS_HEIGHT, CANVAS_WIDTH};
use bevy::prelude::{Resource, Vec2};
use serde::{Deserialize, Serialize};

/// A point in official-chart percentage coordinates (0..1 across the canvas).
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NoisePoint {
    pub x: f32,
    pub y: f32,
}

impl NoisePoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn world(self) -> Vec2 {
        Vec2::new(
            (self.x - 0.5) * CANVAS_WIDTH,
            (self.y - 0.5) * CANVAS_HEIGHT,
        )
    }

    pub fn from_world(point: Vec2) -> Self {
        Self::new(point.x / CANVAS_WIDTH + 0.5, point.y / CANVAS_HEIGHT + 0.5)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NoiseMoveEvent {
    pub beat: Beat,
    pub end_position: NoisePoint,
    pub ease_type_x: u8,
    pub ease_type_y: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NoiseScaleEvent {
    pub beat: Beat,
    pub anchor: NoisePoint,
    pub scale: NoisePoint,
    pub ease_type_x: u8,
    pub ease_type_y: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NoiseRotateEvent {
    pub beat: Beat,
    pub anchor: NoisePoint,
    pub rotation: f32,
    pub ease_type: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseRect {
    pub center: Vec2,
    pub size: Vec2,
    pub rotation_degrees: f32,
}

impl NoiseRect {
    /// Matches Phigros' inclusive rotated-rectangle test. Degenerate domains do not hit.
    pub fn contains(self, point: Vec2) -> bool {
        let half = self.size.abs() / 2.0;
        if half.x <= 0.0 || half.y <= 0.0 {
            return false;
        }
        let local =
            (point - self.center).rotate(Vec2::from_angle(-self.rotation_degrees.to_radians()));
        local.x.abs() <= half.x && local.y.abs() <= half.y
    }
}

/// A Phigros 9 noise domain (`blockAreaList` item).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Component))]
pub struct NoiseArea {
    pub top_right_percentage: NoisePoint,
    pub bottom_left_percentage: NoisePoint,
    pub appear_beat: Beat,
    pub enable_beat: Beat,
    pub disable_beat: Beat,
    pub disappear_beat: Beat,
    pub is_subtract: bool,
    #[serde(default)]
    pub move_events: Vec<NoiseMoveEvent>,
    #[serde(default)]
    pub scale_events: Vec<NoiseScaleEvent>,
    #[serde(default)]
    pub rotate_events: Vec<NoiseRotateEvent>,
}

impl Default for NoiseArea {
    fn default() -> Self {
        Self {
            top_right_percentage: NoisePoint::new(0.75, 0.75),
            bottom_left_percentage: NoisePoint::new(0.25, 0.25),
            appear_beat: Beat::ZERO,
            enable_beat: Beat::ZERO,
            disable_beat: Beat::ONE,
            disappear_beat: Beat::ONE,
            is_subtract: false,
            move_events: vec![],
            scale_events: vec![],
            rotate_events: vec![],
        }
    }
}

impl NoiseArea {
    /// A visible-only (often called "fake") domain has an empty blocking
    /// window. Phigros still renders it through ReadyBlock/DisabledBlock, but
    /// it can never reject a touch.
    pub fn is_visual_only(&self) -> bool {
        self.enable_beat >= self.disable_beat
    }

    pub fn set_visual_only(&mut self, visual_only: bool) {
        if visual_only {
            self.disable_beat = self.enable_beat;
            if self.disappear_beat <= self.disable_beat {
                self.disappear_beat = self.disable_beat + Beat::ONE;
            }
        } else if self.disable_beat <= self.enable_beat {
            self.disable_beat = self.disappear_beat;
            if self.disable_beat <= self.enable_beat {
                self.disable_beat = self.enable_beat + Beat::ONE;
                self.disappear_beat = self.disable_beat;
            }
        }
    }

    /// Converts a desired visual center into the absolute move target used by the official format.
    /// Scale and rotation are evaluated first, so writing the visual center directly would apply
    /// their center offset twice whenever either transform uses an off-center anchor.
    pub fn move_target_for_center(
        &self,
        seconds: f32,
        bpm_list: &BpmList,
        desired_center: Vec2,
    ) -> NoisePoint {
        let base_center = NoisePoint::new(
            (self.top_right_percentage.x + self.bottom_left_percentage.x) / 2.0,
            (self.top_right_percentage.y + self.bottom_left_percentage.y) / 2.0,
        )
        .world();
        let mut without_move = self.clone();
        without_move.move_events.clear();
        let transformed_center = without_move.rect_at(seconds, bpm_list).center;
        NoisePoint::from_world(base_center + desired_center - transformed_center)
    }

    pub fn set_move_key(&mut self, event: NoiseMoveEvent) {
        if let Some(existing) = self
            .move_events
            .iter_mut()
            .rev()
            .find(|existing| existing.beat == event.beat)
        {
            *existing = event;
        } else {
            self.move_events.push(event);
            self.move_events.sort_by_key(|event| event.beat);
        }
    }

    pub fn set_scale_key(&mut self, event: NoiseScaleEvent) {
        if let Some(existing) = self
            .scale_events
            .iter_mut()
            .rev()
            .find(|existing| existing.beat == event.beat)
        {
            *existing = event;
        } else {
            self.scale_events.push(event);
            self.scale_events.sort_by_key(|event| event.beat);
        }
    }

    pub fn set_rotate_key(&mut self, event: NoiseRotateEvent) {
        if let Some(existing) = self
            .rotate_events
            .iter_mut()
            .rev()
            .find(|existing| existing.beat == event.beat)
        {
            *existing = event;
        } else {
            self.rotate_events.push(event);
            self.rotate_events.sort_by_key(|event| event.beat);
        }
    }

    pub fn is_visible(&self, seconds: f32, bpm_list: &BpmList) -> bool {
        bpm_list.time_at(self.appear_beat) <= seconds
            && seconds < bpm_list.time_at(self.disappear_beat)
    }

    pub fn is_active(&self, seconds: f32, bpm_list: &BpmList) -> bool {
        bpm_list.time_at(self.enable_beat) <= seconds
            && seconds < bpm_list.time_at(self.disable_beat)
    }

    pub fn rect_at(&self, seconds: f32, bpm_list: &BpmList) -> NoiseRect {
        let base = NoisePoint::new(
            (self.top_right_percentage.x + self.bottom_left_percentage.x) / 2.0,
            (self.top_right_percentage.y + self.bottom_left_percentage.y) / 2.0,
        )
        .world();
        let mut center = base;
        let mut size = Vec2::new(
            (self.top_right_percentage.x - self.bottom_left_percentage.x).abs() * CANVAS_WIDTH,
            (self.top_right_percentage.y - self.bottom_left_percentage.y).abs() * CANVAS_HEIGHT,
        );
        let mut rotation = 0.0;

        let scale_index = current_index(&self.scale_events, seconds, bpm_list, |e| e.beat);
        if let Some(index) = scale_index {
            let current = self.scale_events[index];
            for pair_index in 1..=index {
                let previous = self.scale_events[pair_index - 1];
                let next = self.scale_events[pair_index];
                center = scale_around(
                    center,
                    previous.anchor.world(),
                    safe_div(next.scale.x, previous.scale.x),
                    safe_div(next.scale.y, previous.scale.y),
                );
            }
            let evaluated_scale = if let Some(following) = self.scale_events.get(index + 1).copied()
            {
                NoisePoint::new(
                    lerp(
                        current.scale.x,
                        following.scale.x,
                        event_progress(
                            current.beat,
                            following.beat,
                            current.ease_type_x,
                            seconds,
                            bpm_list,
                        ),
                    ),
                    lerp(
                        current.scale.y,
                        following.scale.y,
                        event_progress(
                            current.beat,
                            following.beat,
                            current.ease_type_y,
                            seconds,
                            bpm_list,
                        ),
                    ),
                )
            } else {
                current.scale
            };
            center = scale_around(
                center,
                current.anchor.world(),
                safe_div(evaluated_scale.x, current.scale.x),
                safe_div(evaluated_scale.y, current.scale.y),
            );
            size *= Vec2::new(evaluated_scale.x, evaluated_scale.y);
        }

        let rotate_index = current_index(&self.rotate_events, seconds, bpm_list, |e| e.beat);
        if let Some(index) = rotate_index {
            let current = self.rotate_events[index];
            for pair_index in 1..=index {
                let previous = self.rotate_events[pair_index - 1];
                let next = self.rotate_events[pair_index];
                center = rotate_around(
                    center,
                    previous.anchor.world(),
                    next.rotation - previous.rotation,
                );
            }
            rotation = if let Some(following) = self.rotate_events.get(index + 1).copied() {
                lerp(
                    current.rotation,
                    following.rotation,
                    event_progress(
                        current.beat,
                        following.beat,
                        current.ease_type,
                        seconds,
                        bpm_list,
                    ),
                )
            } else {
                current.rotation
            };
            center = rotate_around(center, current.anchor.world(), rotation - current.rotation);
        }

        let move_index = current_index(&self.move_events, seconds, bpm_list, |e| e.beat);
        if let Some(index) = move_index {
            let current = self.move_events[index];
            let target = if let Some(following) = self.move_events.get(index + 1).copied() {
                NoisePoint::new(
                    lerp(
                        current.end_position.x,
                        following.end_position.x,
                        event_progress(
                            current.beat,
                            following.beat,
                            current.ease_type_x,
                            seconds,
                            bpm_list,
                        ),
                    ),
                    lerp(
                        current.end_position.y,
                        following.end_position.y,
                        event_progress(
                            current.beat,
                            following.beat,
                            current.ease_type_y,
                            seconds,
                            bpm_list,
                        ),
                    ),
                )
                .world()
            } else {
                current.end_position.world()
            };
            center += target - base;
        }

        NoiseRect {
            center,
            size: size.abs(),
            rotation_degrees: rotation,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, Resource)]
pub struct NoiseAreas(pub Vec<NoiseArea>);

/// Whether a point is blocked at this instant.
///
/// Normal domains form a union; subtract domains toggle the result by parity.
pub fn hit_test(areas: &NoiseAreas, point: Vec2, seconds: f32, bpm_list: &BpmList) -> bool {
    let mut has_normal = false;
    let mut subtract_count = 0usize;
    for area in &areas.0 {
        if !area.is_active(seconds, bpm_list) || !area.rect_at(seconds, bpm_list).contains(point) {
            continue;
        }
        if area.is_subtract {
            subtract_count += 1;
        } else {
            has_normal = true;
        }
    }
    has_normal ^ (subtract_count % 2 == 1)
}

fn current_index<T>(
    events: &[T],
    seconds: f32,
    bpm_list: &BpmList,
    beat: impl Fn(&T) -> Beat,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .take_while(|(_, event)| bpm_list.time_at(beat(event)) <= seconds)
        .map(|(index, _)| index)
        .last()
}

fn event_progress(from: Beat, to: Beat, ease_type: u8, seconds: f32, bpm_list: &BpmList) -> f32 {
    let start = bpm_list.time_at(from);
    let end = bpm_list.time_at(to);
    let raw = if end == start {
        1.0
    } else {
        (seconds - start) / (end - start)
    };
    phigros_ease(raw, ease_type).clamp(0.0, 1.0)
}

fn phigros_ease(progress: f32, ease_type: u8) -> f32 {
    fn sample(index: usize, power: i32, kind: u8) -> f32 {
        let t = index as f32 / 100.0;
        match kind {
            0 => t.powi(power),
            1 => 1.0 - (1.0 - t).powi(power),
            _ if index < 50 => {
                let shifted = (index * 2 + 8).min(100) as f32 / 100.0;
                0.5 * shifted.powi(power)
            }
            _ if index < 100 => {
                let shifted = (index * 2 - 92).min(100) as f32 / 100.0;
                let ease_in = shifted.powi(power);
                0.5 + 0.5 * (1.0 - (1.0 - ease_in).powi(power))
            }
            _ => 1.0,
        }
    }

    let progress = progress.clamp(0.0, 1.0);
    if ease_type == 13 {
        return 0.0;
    }
    if ease_type == 14 {
        return 1.0;
    }
    if ease_type == 0 || ease_type > 14 {
        return progress;
    }
    let group = (ease_type - 1) / 3;
    let kind = (ease_type - 1) % 3;
    let power = group as i32 + 2;
    let position = progress * 100.0;
    let index = position.floor() as usize;
    if index >= 100 {
        return sample(100, power, kind);
    }
    lerp(
        sample(index, power, kind),
        sample(index + 1, power, kind),
        position - index as f32,
    )
}

fn lerp(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

fn safe_div(numerator: f32, denominator: f32) -> f32 {
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

fn scale_around(point: Vec2, anchor: Vec2, x: f32, y: f32) -> Vec2 {
    anchor + (point - anchor) * Vec2::new(x, y)
}

fn rotate_around(point: Vec2, anchor: Vec2, degrees: f32) -> Vec2 {
    anchor + (point - anchor).rotate(Vec2::from_angle(degrees.to_radians()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beat;

    #[test]
    fn active_window_is_left_closed_right_open() {
        let area = NoiseArea::default();
        let bpm = BpmList::single(60.0);
        assert!(area.is_active(0.0, &bpm));
        assert!(area.is_active(0.999, &bpm));
        assert!(!area.is_active(1.0, &bpm));
    }

    #[test]
    fn visual_only_domain_stays_visible_without_blocking() {
        let bpm = BpmList::single(60.0);
        let mut area = NoiseArea {
            disappear_beat: crate::beat!(4),
            ..NoiseArea::default()
        };
        area.set_visual_only(true);

        assert!(area.is_visual_only());
        assert!(area.is_visible(0.5, &bpm));
        assert!(!area.is_active(0.0, &bpm));
        assert!(!hit_test(&NoiseAreas(vec![area]), Vec2::ZERO, 0.5, &bpm));
    }

    #[test]
    fn subtract_domains_toggle_by_parity() {
        let bpm = BpmList::single(60.0);
        let normal = NoiseArea::default();
        let mut subtract = normal.clone();
        subtract.is_subtract = true;
        let center = Vec2::ZERO;
        assert!(hit_test(
            &NoiseAreas(vec![normal.clone()]),
            center,
            0.5,
            &bpm
        ));
        assert!(!hit_test(
            &NoiseAreas(vec![normal.clone(), subtract.clone()]),
            center,
            0.5,
            &bpm
        ));
        assert!(hit_test(
            &NoiseAreas(vec![normal, subtract.clone(), subtract]),
            center,
            0.5,
            &bpm
        ));
    }

    #[test]
    fn rectangle_edges_are_inclusive() {
        let area = NoiseArea::default();
        let bpm = BpmList::single(60.0);
        let rect = area.rect_at(0.5, &bpm);
        assert!(rect.contains(Vec2::new(rect.size.x / 2.0, 0.0)));
        assert!(!rect.contains(Vec2::new(rect.size.x / 2.0 + 0.01, 0.0)));
    }

    #[test]
    fn move_target_compensates_for_anchored_transforms() {
        let bpm = BpmList::single(60.0);
        let mut area = NoiseArea {
            scale_events: vec![
                NoiseScaleEvent {
                    beat: Beat::ZERO,
                    anchor: NoisePoint::new(0.5, 0.5),
                    scale: NoisePoint::new(1.0, 1.0),
                    ease_type_x: 0,
                    ease_type_y: 0,
                },
                NoiseScaleEvent {
                    beat: Beat::ONE,
                    anchor: NoisePoint::new(0.2, 0.3),
                    scale: NoisePoint::new(1.5, 0.75),
                    ease_type_x: 0,
                    ease_type_y: 0,
                },
            ],
            rotate_events: vec![
                NoiseRotateEvent {
                    beat: Beat::ZERO,
                    anchor: NoisePoint::new(0.5, 0.5),
                    rotation: 0.0,
                    ease_type: 0,
                },
                NoiseRotateEvent {
                    beat: Beat::ONE,
                    anchor: NoisePoint::new(0.8, 0.2),
                    rotation: 30.0,
                    ease_type: 0,
                },
            ],
            ..NoiseArea::default()
        };
        let desired = Vec2::new(123.0, -45.0);
        let target = area.move_target_for_center(1.0, &bpm, desired);
        area.set_move_key(NoiseMoveEvent {
            beat: Beat::ONE,
            end_position: target,
            ease_type_x: 0,
            ease_type_y: 0,
        });
        let actual = area.rect_at(1.0, &bpm).center;
        assert!((actual - desired).length() < 0.001, "{actual:?}");
    }

    #[test]
    fn duplicate_beat_move_event_jumps_then_continues() {
        let bpm = BpmList::single(60.0);
        let area = NoiseArea {
            move_events: vec![
                NoiseMoveEvent {
                    beat: beat!(0),
                    end_position: NoisePoint::new(0.5, 0.5),
                    ease_type_x: 0,
                    ease_type_y: 0,
                },
                NoiseMoveEvent {
                    beat: beat!(1),
                    end_position: NoisePoint::new(0.5, 1.5),
                    ease_type_x: 0,
                    ease_type_y: 0,
                },
                NoiseMoveEvent {
                    beat: beat!(1),
                    end_position: NoisePoint::new(0.5, -1.5),
                    ease_type_x: 0,
                    ease_type_y: 0,
                },
                NoiseMoveEvent {
                    beat: beat!(2),
                    end_position: NoisePoint::new(0.5, 0.5),
                    ease_type_x: 0,
                    ease_type_y: 0,
                },
            ],
            ..NoiseArea::default()
        };

        assert!((area.rect_at(1.0, &bpm).center.y + 2.0 * CANVAS_HEIGHT).abs() < 0.001);
        assert!((area.rect_at(1.5, &bpm).center.y + CANVAS_HEIGHT).abs() < 0.001);
        assert!(area.rect_at(2.0, &bpm).center.y.abs() < 0.001);
    }

    #[test]
    fn scale_and_rotation_interpolate_after_same_beat_jump() {
        let bpm = BpmList::single(60.0);
        let center = NoisePoint::new(0.5, 0.5);
        let area = NoiseArea {
            scale_events: vec![
                NoiseScaleEvent {
                    beat: beat!(0),
                    anchor: center,
                    scale: NoisePoint::new(1.0, 1.0),
                    ease_type_x: 0,
                    ease_type_y: 0,
                },
                NoiseScaleEvent {
                    beat: beat!(1),
                    anchor: center,
                    scale: NoisePoint::new(0.5, 0.5),
                    ease_type_x: 0,
                    ease_type_y: 0,
                },
                NoiseScaleEvent {
                    beat: beat!(1),
                    anchor: center,
                    scale: NoisePoint::new(1.0, 1.0),
                    ease_type_x: 0,
                    ease_type_y: 0,
                },
                NoiseScaleEvent {
                    beat: beat!(2),
                    anchor: center,
                    scale: NoisePoint::new(2.0, 2.0),
                    ease_type_x: 0,
                    ease_type_y: 0,
                },
            ],
            rotate_events: vec![
                NoiseRotateEvent {
                    beat: beat!(0),
                    anchor: center,
                    rotation: 0.0,
                    ease_type: 0,
                },
                NoiseRotateEvent {
                    beat: beat!(1),
                    anchor: center,
                    rotation: 90.0,
                    ease_type: 0,
                },
                NoiseRotateEvent {
                    beat: beat!(1),
                    anchor: center,
                    rotation: 0.0,
                    ease_type: 0,
                },
                NoiseRotateEvent {
                    beat: beat!(2),
                    anchor: center,
                    rotation: 90.0,
                    ease_type: 0,
                },
            ],
            ..NoiseArea::default()
        };

        let base_size = NoiseArea::default().rect_at(0.0, &bpm).size;
        let at_jump = area.rect_at(1.0, &bpm);
        assert!((at_jump.size - base_size).length() < 0.001);
        assert!(at_jump.rotation_degrees.abs() < 0.001);

        let halfway = area.rect_at(1.5, &bpm);
        assert!((halfway.size - base_size * 1.5).length() < 0.001);
        assert!((halfway.rotation_degrees - 45.0).abs() < 0.001);

        let end = area.rect_at(2.0, &bpm);
        assert!((end.size - base_size * 2.0).length() < 0.001);
        assert!((end.rotation_degrees - 90.0).abs() < 0.001);
    }
}
