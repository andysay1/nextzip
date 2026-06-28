use std::collections::BTreeMap;

use serde_json::Value;

pub fn flatten_json(value: &Value) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    flatten_into("", value, &mut out);
    out
}

fn flatten_into(prefix: &str, value: &Value, out: &mut BTreeMap<String, Value>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let name = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_into(&name, child, out);
            }
        }
        _ => {
            out.insert(prefix.to_string(), value.clone());
        }
    }
}
