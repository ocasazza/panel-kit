use serde_json::Value;

/// Return all concrete JSON leaf pointers in document order.
pub(crate) fn schema_leaf_pointers(value: &Value) -> Vec<String> {
    let mut pointers = Vec::new();
    collect_leaf_pointers(value, "", &mut pointers);
    pointers
}

fn collect_leaf_pointers(value: &Value, pointer: &str, pointers: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                collect_leaf_pointers(child, &join_pointer(pointer, key), pointers);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_leaf_pointers(child, &join_pointer(pointer, &index.to_string()), pointers);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            pointers.push(pointer.to_owned());
        }
    }
}

fn join_pointer(base: &str, segment: &str) -> String {
    let escaped = segment.replace('~', "~0").replace('/', "~1");
    if base.is_empty() {
        format!("/{escaped}")
    } else {
        format!("{base}/{escaped}")
    }
}
