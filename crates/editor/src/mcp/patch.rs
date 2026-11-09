//! JSON merge patches (RFC 7396) on the engine's types, for the agents' `update_*` / `upsert_def`
//! / `set_*` tools: a patch is an object whose fields replace the target's, `null` removes a
//! field, nested objects merge, anything else (arrays, scalars) replaces.

use rustpg2d_engine::entity::EntityInstance;
use rustpg2d_engine::map::Zone;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Applies `patch` to `target` in place.
pub fn merge(target: &mut Value, patch: &Value) {
    match patch {
        Value::Object(p) => {
            if !target.is_object() {
                *target = Value::Object(Default::default());
            }
            let t = target.as_object_mut().unwrap();
            for (k, v) in p {
                if v.is_null() {
                    t.remove(k);
                } else {
                    merge(t.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
        }
        _ => *target = patch.clone(),
    }
}

/// `value` with `patch` applied, read back as the same type (an error names the field that
/// doesn't fit).
pub fn apply<T: Serialize + DeserializeOwned>(value: &T, patch: &Value) -> Result<T, String> {
    if !patch.is_object() {
        return Err("the patch must be a JSON object of fields to change".into());
    }
    let mut v = serde_json::to_value(value).map_err(|e| e.to_string())?;
    merge(&mut v, patch);
    serde_json::from_value(v).map_err(|e| format!("the patched value isn't valid: {e}"))
}

/// Entity JSON for agents: the engine's fields plus `tile_x` / `tile_y` (the tile the feet are on).
pub fn entity_json(e: &EntityInstance, ts: f32) -> Value {
    let mut v = serde_json::to_value(e).unwrap_or(Value::Null);
    if let Value::Object(m) = &mut v {
        let (tx, ty) = tile_of(e, ts);
        m.insert("tile_x".into(), tx.into());
        m.insert("tile_y".into(), ty.into());
    }
    v
}

/// The tile an entity stands on ([`rustpg2d_engine::grid::feet_tile`]).
pub fn tile_of(e: &EntityInstance, ts: f32) -> (i32, i32) {
    rustpg2d_engine::grid::feet_tile(macroquad::math::vec2(e.x, e.y), ts)
}

/// Feet position (pixels) of an entity standing on tile `(tx, ty)` ([`rustpg2d_engine::grid::feet_spot`]).
pub fn feet_of(tx: i32, ty: i32, ts: f32) -> (f32, f32) {
    let p = rustpg2d_engine::grid::feet_spot((tx, ty), ts);
    (p.x, p.y)
}

/// An entity with `patch` applied: `tile_x` / `tile_y` move it, `uid` can't change.
pub fn patch_entity(e: &EntityInstance, patch: &Value, ts: f32) -> Result<EntityInstance, String> {
    let Value::Object(p) = patch else { return Err("the patch must be a JSON object".into()) };
    if p.get("uid").is_some_and(|u| u.as_u64() != Some(e.uid as u64)) {
        return Err("uid can't change".into());
    }
    let mut p = p.clone();
    p.remove("uid");
    let (mut tx, mut ty) = tile_of(e, ts);
    let mut moved = false;
    for (k, t) in [("tile_x", &mut tx), ("tile_y", &mut ty)] {
        if let Some(v) = p.remove(k) {
            *t = v.as_i64().ok_or_else(|| format!("{k} must be a whole number"))? as i32;
            moved = true;
        }
    }
    let mut out: EntityInstance = apply(e, &Value::Object(p))?;
    if moved {
        let (x, y) = feet_of(tx, ty, ts);
        out.x = x;
        out.y = y;
    }
    out.uid = e.uid;
    Ok(out)
}

/// A zone with `patch` applied; `uid` can't change.
pub fn patch_zone(z: &Zone, patch: &Value) -> Result<Zone, String> {
    let Value::Object(p) = patch else { return Err("the patch must be a JSON object".into()) };
    if p.get("uid").is_some_and(|u| u.as_u64() != Some(z.uid as u64)) {
        return Err("uid can't change".into());
    }
    let mut p = p.clone();
    p.remove("uid");
    let mut out: Zone = apply(z, &Value::Object(p))?;
    out.uid = z.uid;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::entity::EntityKind;
    use serde_json::json;

    #[test]
    fn merge_follows_rfc_7396() {
        let mut t = json!({"a": {"b": 1, "c": 2}, "d": [1, 2], "e": "x"});
        merge(&mut t, &json!({"a": {"b": null, "n": 3}, "d": [9], "f": true}));
        assert_eq!(t, json!({"a": {"c": 2, "n": 3}, "d": [9], "e": "x", "f": true}));
        let mut s = json!("scalar");
        merge(&mut s, &json!({"k": 1}));
        assert_eq!(s, json!({"k": 1}));
    }

    #[test]
    fn entities_move_by_tile_and_keep_their_uid() {
        let ts = 32.0;
        let e = EntityInstance::new(7, EntityKind::Prop, "barrel", 48.0, 63.0);
        assert_eq!(tile_of(&e, ts), (1, 1));
        let v = entity_json(&e, ts);
        assert_eq!((v["tile_x"].clone(), v["tile_y"].clone()), (json!(1), json!(1)));
        let moved = patch_entity(&e, &json!({"tile_x": 4, "tile_y": 2, "name": "keg"}), ts).unwrap();
        assert_eq!((moved.x, moved.y), feet_of(4, 2, ts));
        assert_eq!((moved.uid, moved.name.as_str()), (7, "keg"));
        assert!(patch_entity(&e, &json!({"uid": 9}), ts).is_err());
        assert!(patch_entity(&e, &json!({"facing": "Sideways"}), ts).is_err(), "unknown enum values are errors");
        assert!(apply(&e, &json!([1])).is_err(), "a patch must be an object");
    }
}
