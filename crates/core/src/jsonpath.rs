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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "key": "PROJ-1",
            "fields": { "status": { "name": "In Progress" }, "votes": 3, "done": false, "parent": null },
            "labels": ["bug", "ui"],
            "components": [{ "name": "core" }, { "name": "cli" }],
            "items": [{ "key": "A" }, { "key": "B" }]
        })
    }

    #[test]
    fn get_resolves_nested_keys_and_array_indices() {
        let v = sample();
        assert_eq!(get(&v, "key"), Some(&json!("PROJ-1")));
        assert_eq!(get(&v, "fields.status.name"), Some(&json!("In Progress")));
        assert_eq!(get(&v, "items.1.key"), Some(&json!("B")));
        // Empty segments (leading dot / `$.`-style) are skipped.
        assert_eq!(get(&v, ".key"), Some(&json!("PROJ-1")));
    }

    #[test]
    fn get_returns_none_for_missing_or_wrong_shape() {
        let v = sample();
        assert_eq!(get(&v, "nope"), None);
        assert_eq!(get(&v, "fields.missing.x"), None);
        assert_eq!(get(&v, "items.9"), None); // index out of range
        assert_eq!(get(&v, "items.notanindex"), None); // non-numeric index
        assert_eq!(get(&v, "key.deeper"), None); // descend into a scalar
    }

    #[test]
    fn get_str_coerces_scalars_and_rejects_null_and_containers() {
        let v = sample();
        assert_eq!(
            get_str(&v, "fields.status.name").as_deref(),
            Some("In Progress")
        );
        assert_eq!(get_str(&v, "fields.votes").as_deref(), Some("3"));
        assert_eq!(get_str(&v, "fields.done").as_deref(), Some("false"));
        assert_eq!(get_str(&v, "fields.parent"), None); // null -> None
        assert_eq!(get_str(&v, "fields"), None); // object isn't a scalar
        assert_eq!(get_str(&v, "missing"), None);
    }

    #[test]
    fn get_str_array_handles_strings_objects_and_non_arrays() {
        let v = sample();
        assert_eq!(get_str_array(&v, "labels"), vec!["bug", "ui"]);
        // Objects coerce via their `name` field (labels/components shape).
        assert_eq!(get_str_array(&v, "components"), vec!["core", "cli"]);
        assert!(get_str_array(&v, "missing").is_empty());
        assert!(get_str_array(&v, "key").is_empty()); // not an array
    }

    #[test]
    fn get_array_supports_bare_top_level_and_dollar_prefix() {
        let bare = json!([{ "key": "X" }]);
        assert_eq!(get_array(&bare, "").map(Vec::len), Some(1));
        assert_eq!(get_array(&bare, "$").map(Vec::len), Some(1));

        let v = sample();
        assert_eq!(get_array(&v, "$.items").map(Vec::len), Some(2));
        assert_eq!(get_array(&v, "items").map(Vec::len), Some(2));
        assert_eq!(get_array(&v, "key"), None); // scalar isn't an array
        assert_eq!(get_array(&v, "missing"), None);
    }
}
