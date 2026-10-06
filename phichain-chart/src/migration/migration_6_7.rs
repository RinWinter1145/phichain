use crate::migration::Migration;
use serde_json::{json, Value};

/// Adds the top-level `noise_areas` collection used for Phigros 9 block areas.
pub struct Migration6To7;

impl Migration for Migration6To7 {
    fn migrate(old: &Value) -> anyhow::Result<Value> {
        let mut chart = old.clone();
        chart["noise_areas"] = json!([]);
        chart["format"] = json!(7);
        Ok(chart)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_empty_noise_area_list() {
        let old = json!({"format": 6, "offset": 0.0, "bpm_list": [], "lines": []});
        let migrated = Migration6To7::migrate(&old).unwrap();
        assert_eq!(migrated["format"], 7);
        assert_eq!(migrated["noise_areas"], json!([]));
    }
}
