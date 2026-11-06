//! Provider-neutral question validation, shared with native adapters.
use caelum_protocol::QuestionPrompt;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Answer {
    pub values: Map<String, Value>,
    pub errors: BTreeMap<String, String>,
    pub active: Vec<String>,
}

pub fn defaults(prompt: &QuestionPrompt) -> Map<String, Value> {
    prompt
        .fields
        .iter()
        .filter_map(|field| Some((field["key"].as_str()?.into(), field.get("default")?.clone())))
        .collect()
}

pub fn validate(prompt: &QuestionPrompt, draft: &Map<String, Value>) -> Answer {
    let mut result = Answer::default();
    for field in &prompt.fields {
        let Some(key) = field["key"].as_str() else {
            continue;
        };
        if field["when"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|condition| {
                let value = result
                    .values
                    .get(condition["key"].as_str().unwrap_or_default());
                let matches = value.is_some_and(|v| {
                    same_value(v, &condition["value"])
                        || v.as_array()
                            .is_some_and(|v| v.iter().any(|v| same_value(v, &condition["value"])))
                });
                value.is_none()
                    || if condition["op"] == "eq" {
                        !matches
                    } else {
                        matches
                    }
            })
        {
            continue;
        }
        result.active.push(key.into());
        if field["type"] == "external" {
            continue;
        }
        let mut value = draft.get(key).cloned().unwrap_or(Value::Null);
        let required = field["required"] == true;
        let missing = value.is_null() || value == "";
        if missing && !required {
            continue;
        }
        let fail = |message: &str| -> Result<(), String> { Err(message.into()) };
        let checked = (|| {
            if missing {
                return fail("Please answer this question.");
            }
            match field["type"].as_str().unwrap_or_default() {
                "number" | "integer" => {
                    let number = value
                        .as_f64()
                        .or_else(|| value.as_str()?.parse::<f64>().ok())
                        .filter(|v| v.is_finite())
                        .ok_or("Enter a number.")?;
                    if field["type"] == "integer" && number.fract() != 0.0 {
                        return fail("Enter a whole number.");
                    }
                    if field["minimum"].as_f64().is_some_and(|v| number < v)
                        || field["maximum"].as_f64().is_some_and(|v| number > v)
                    {
                        return fail("Value is outside the allowed range.");
                    }
                    value = if field["type"] == "integer"
                        && number >= i64::MIN as f64
                        && number < i64::MAX as f64
                    {
                        serde_json::json!(number as i64)
                    } else {
                        serde_json::json!(number)
                    };
                }
                "boolean" => {
                    if !value.is_boolean() {
                        return fail("Choose Yes or No.");
                    }
                }
                "string" => {
                    let text = value.as_str().ok_or("Enter text.")?;
                    if required && text.trim().is_empty() {
                        return fail("Please answer this question.");
                    }
                    let len = text.encode_utf16().count() as u64;
                    if field["minLength"].as_u64().is_some_and(|v| len < v)
                        || field["maxLength"].as_u64().is_some_and(|v| len > v)
                    {
                        return fail("Text length is outside the allowed range.");
                    }
                    if let Some(pattern) = field["pattern"].as_str() {
                        if !regex::Regex::new(pattern)
                            .map_err(|_| "Unsupported validation pattern.")?
                            .is_match(text)
                        {
                            return fail("Use the requested format.");
                        }
                    }
                    if field["custom"] != true
                        && field["options"].as_array().is_some_and(|options| {
                            !options.is_empty()
                                && !options.iter().any(|option| option["value"] == value)
                        })
                    {
                        return fail("Choose an available answer.");
                    }
                }
                "multiselect" => {
                    let values = value.as_array_mut().ok_or("Choose available answers.")?;
                    if values.iter().any(|v| !v.is_string()) {
                        return fail("Choose text answers.");
                    }
                    let mut unique = Vec::new();
                    for v in values.drain(..) {
                        if !unique.contains(&v) {
                            unique.push(v);
                        }
                    }
                    *values = unique;
                    let min = field["minItems"].as_u64().unwrap_or(u64::from(required));
                    if (values.len() as u64) < min
                        || field["maxItems"]
                            .as_u64()
                            .is_some_and(|v| values.len() as u64 > v)
                    {
                        return fail("Choose the requested number of answers.");
                    }
                    if field["custom"] != true
                        && values.iter().any(|v| {
                            !field["options"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .any(|o| o["value"] == *v)
                        })
                    {
                        return fail("Choose available answers.");
                    }
                }
                _ => return fail("Unsupported question type."),
            }
            Ok(())
        })();
        match checked {
            Ok(()) => {
                result.values.insert(key.into(), value);
            }
            Err(error) => {
                result.errors.insert(key.into(), error);
            }
        }
    }
    result
}

fn same_value(left: &Value, right: &Value) -> bool {
    left == right
        || left
            .as_f64()
            .zip(right.as_f64())
            .is_some_and(|(a, b)| a == b)
}
