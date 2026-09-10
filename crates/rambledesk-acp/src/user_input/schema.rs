use crate::AcpError;
use serde_json::Value;

/// Support the flat form vocabulary the composer can render. Unknown assertion
/// keywords are rejected; they must never silently weaken an agent's schema.
pub(super) fn check_schema(schema: &Value, depth: usize) -> Result<(), AcpError> {
    let object = schema.as_object().ok_or(AcpError::InvalidPermission)?;
    if depth > 4 {
        return Err(AcpError::InvalidPermission);
    }
    for key in object.keys() {
        if ![
            "$schema",
            "type",
            "title",
            "description",
            "default",
            "_meta",
            "properties",
            "required",
            "additionalProperties",
            "items",
            "enum",
            "enumNames",
            "oneOf",
            "anyOf",
            "const",
            "minimum",
            "maximum",
            "exclusiveMinimum",
            "exclusiveMaximum",
            "minLength",
            "maxLength",
            "minItems",
            "maxItems",
            "uniqueItems",
            "x-rambledesk-allow-other",
        ]
        .contains(&key.as_str())
        {
            return Err(AcpError::InvalidPermission);
        }
    }
    if depth == 0 {
        // The supported UI is a flat object. Do not allow nested assertions to
        // bypass the field-schema inspection through root combinators.
        if schema.get("oneOf").is_some()
            || schema.get("anyOf").is_some()
            || schema.get("items").is_some()
        {
            return Err(AcpError::InvalidPermission);
        }
        if schema["type"] != "object" {
            return Err(AcpError::InvalidPermission);
        }
        let props = schema
            .get("properties")
            .and_then(Value::as_object)
            .ok_or(AcpError::InvalidPermission)?;
        if props.len() > 64 {
            return Err(AcpError::InvalidPermission);
        }
        for prop in props.values() {
            check_schema(prop, depth + 1)?;
        }
        if let Some(required) = schema.get("required") {
            let required = required.as_array().ok_or(AcpError::InvalidPermission)?;
            if required
                .iter()
                .any(|v| v.as_str().is_none_or(|key| !props.contains_key(key)))
            {
                return Err(AcpError::InvalidPermission);
            }
        }
        if schema
            .get("additionalProperties")
            .is_some_and(|v| !v.is_boolean())
        {
            return Err(AcpError::InvalidPermission);
        }
    } else {
        let kind = schema.get("type").and_then(Value::as_str);
        if !matches!(
            kind,
            Some("string" | "boolean" | "number" | "integer" | "array") | None
        ) {
            return Err(AcpError::InvalidPermission);
        }
        if kind.is_none()
            && !schema.get("const").is_some_and(Value::is_string)
            && schema.get("enum").is_none()
            && schema.get("anyOf").is_none()
        {
            return Err(AcpError::InvalidPermission);
        }
        if kind == Some("array") {
            check_schema(
                schema.get("items").ok_or(AcpError::InvalidPermission)?,
                depth + 1,
            )?;
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(list) = schema.get(key) {
                let list = list
                    .as_array()
                    .filter(|a| !a.is_empty() && a.len() <= 128)
                    .ok_or(AcpError::InvalidPermission)?;
                for item in list {
                    check_schema(item, depth + 1)?;
                }
            }
        }
        if let Some(values) = schema.get("enum")
            && values
                .as_array()
                .is_none_or(|a| a.is_empty() || !a.iter().all(Value::is_string))
        {
            return Err(AcpError::InvalidPermission);
        }
    }
    for key in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
        if schema.get(key).is_some_and(|v| !v.is_number()) {
            return Err(AcpError::InvalidPermission);
        }
    }
    for key in ["uniqueItems", "x-rambledesk-allow-other"] {
        if schema.get(key).is_some_and(|v| !v.is_boolean()) {
            return Err(AcpError::InvalidPermission);
        }
    }
    for key in ["minLength", "maxLength", "minItems", "maxItems"] {
        if schema.get(key).is_some_and(|v| v.as_u64().is_none()) {
            return Err(AcpError::InvalidPermission);
        }
    }
    Ok(())
}

pub(super) fn validate(schema: &Value, value: &Value) -> Result<(), AcpError> {
    let invalid = || AcpError::InvalidPermission;
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => {
            let values = value.as_object().ok_or_else(invalid)?;
            let properties = schema["properties"].as_object().ok_or_else(invalid)?;
            if values.keys().any(|key| !properties.contains_key(key)) {
                return Err(invalid());
            }
            if schema
                .get("required")
                .and_then(Value::as_array)
                .is_some_and(|keys| {
                    keys.iter()
                        .any(|k| !values.contains_key(k.as_str().unwrap_or("")))
                })
            {
                return Err(invalid());
            }
            for (key, value) in values {
                validate(&properties[key], value)?;
            }
        }
        Some("string") if !value.is_string() => return Err(invalid()),
        Some("boolean") if !value.is_boolean() => return Err(invalid()),
        Some("number") if !value.is_number() => return Err(invalid()),
        Some("integer") if !value.is_i64() && !value.is_u64() => return Err(invalid()),
        Some("array") => {
            let values = value.as_array().ok_or_else(invalid)?;
            bounds(schema, "minItems", "maxItems", values.len() as f64)?;
            for (index, value) in values.iter().enumerate() {
                if schema["uniqueItems"] == true && values[..index].contains(value) {
                    return Err(invalid());
                }
                validate(&schema["items"], value)?;
            }
        }
        _ => {}
    }
    if let Some(text) = value.as_str() {
        bounds(
            schema,
            "minLength",
            "maxLength",
            text.chars().count() as f64,
        )?;
    }
    if let Some(number) = value.as_f64() {
        bounds(schema, "minimum", "maximum", number)?;
        if schema
            .get("exclusiveMinimum")
            .and_then(Value::as_f64)
            .is_some_and(|n| number <= n)
            || schema
                .get("exclusiveMaximum")
                .and_then(Value::as_f64)
                .is_some_and(|n| number >= n)
        {
            return Err(invalid());
        }
    }
    let custom = schema["x-rambledesk-allow-other"] == true
        && value.as_str().is_some_and(|s| !s.trim().is_empty());
    if !custom {
        if schema.get("const").is_some_and(|v| v != value) {
            return Err(invalid());
        }
        if schema
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.contains(value))
        {
            return Err(invalid());
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(options) = schema.get(key).and_then(Value::as_array) {
                let count = options
                    .iter()
                    .filter(|option| validate(option, value).is_ok())
                    .count();
                if count == 0 || key == "oneOf" && count != 1 {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}

fn bounds(schema: &Value, min: &str, max: &str, value: f64) -> Result<(), AcpError> {
    if schema
        .get(min)
        .and_then(Value::as_f64)
        .is_some_and(|n| value < n)
        || schema
            .get(max)
            .and_then(Value::as_f64)
            .is_some_and(|n| value > n)
    {
        Err(AcpError::InvalidPermission)
    } else {
        Ok(())
    }
}
