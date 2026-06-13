//! Tiny dotted-path resolver over `serde_json::Value`.
//!
//! Used to read issues out of whatever JSON the tracker's `fetch` command emits,
//! without the engine knowing that tracker's shape (§5, §11). Supports object
//! keys and numeric array indices: `fields.status.name`, `items.0.key`.

use serde_json::Value;

/// Resolve a dotted path. Returns `None` if any segment is missing.
pub fn get<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = value;
    for seg in path.split('.') {
        if seg.is_empty() {
            continue;
        }
        cur = match cur {
            Value::Object(map) => map.get(seg)?,
            Value::Array(arr) => {
                let idx: usize = seg.parse().ok()?;
                arr.get(idx)?
            }
            _ => return None,
        };
    }
    Some(cur)
}

/// Resolve a path to a `String`, coercing scalars. `None` if missing or null.
pub fn get_str(value: &Value, path: &str) -> Option<String> {
    match get(value, path)? {
        Value::String(s) => Some(s.clone()),
        Value::Null => None,
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        // Objects/arrays aren't meaningful as a scalar field; ignore.
        _ => None,
    }
}

/// Resolve a path to an array of strings. Empty if missing or not an array.
/// Array elements may be plain strings or objects with a `name` field (a common
/// shape for labels/components across trackers).
pub fn get_str_array(value: &Value, path: &str) -> Vec<String> {
    match get(value, path) {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Object(_) => get_str(v, "name"),
                Value::Null => None,
                other => Some(other.to_string()),
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Resolve the items array. If `path` is empty or `$`, the value itself must be
/// the array (some trackers return a bare top-level list).
pub fn get_array<'a>(value: &'a Value, path: &str) -> Option<&'a Vec<Value>> {
    let target = if path.is_empty() || path == "$" {
        value
    } else {
        get(value, path.trim_start_matches("$."))?
    };
    target.as_array()
}
