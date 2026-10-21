//! Opt-in page transport. Parsing also builds natively (K-41).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
mod bridge;
#[cfg(target_arch = "wasm32")]
mod files;
#[cfg(target_arch = "wasm32")]
pub use bridge::{attach, runner};

pub const MAX_ORDINARY_BYTES: usize = 1024 * 1024;
pub const MAX_CLICK_COUNT: u64 = 256;
pub const MAX_DRAG_STEPS: u64 = 32;

pub fn parse(text: &str) -> Result<(String, Value), String> {
    if text.len() > deckcraft_ui_egui::c0_host::MAX_REQUEST_BYTES {
        return Err("request exceeds the host request limit".into());
    }
    let request: Value = serde_json::from_str(text).map_err(|e| format!("request is not JSON: {e}"))?;
    let method = request.get("method").and_then(Value::as_str).ok_or("request has no method")?;
    if !method.starts_with("c0.") && text.len() > MAX_ORDINARY_BYTES {
        return Err("request is larger than 1 MiB".into());
    }
    let params = request.get("params").filter(|p| !p.is_null()).cloned().unwrap_or(json!({}));
    if !params.is_object() {
        return Err("params must be an object".into());
    }
    let bound = match method {
        "ui.click" => Some(("count", MAX_CLICK_COUNT)),
        "ui.drag" => Some(("steps", MAX_DRAG_STEPS)),
        _ => None,
    };
    if let Some((key, limit)) = bound
        && params.get(key).and_then(Value::as_u64).is_some_and(|v| v > limit)
    {
        return Err(format!("{key} exceeds the page input limit of {limit}"));
    }
    Ok((method.into(), params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_missing_or_null_params() {
        assert_eq!(parse(r#"{"method":"c0.inspect"}"#), Ok(("c0.inspect".into(), json!({}))));
        assert_eq!(parse(r#"{"method":"c0.inspect","params":null}"#), Ok(("c0.inspect".into(), json!({}))));
    }
    #[test]
    fn rejects_malformed_envelopes() {
        for text in ["broken", "[]", "{}", r#"{"method":4}"#, r#"{"method":"c0.inspect","params":[]}"#] {
            assert!(parse(text).is_err());
        }
    }
    #[test]
    fn ordinary_limit_does_not_reject_host_file_envelopes() {
        let request = json!({"method":"ui.text", "params":{"text":"a".repeat(MAX_ORDINARY_BYTES)}}).to_string();
        assert!(parse(&request).is_err());
        assert!(parse(&request.replace("ui.text", "c0.openFile")).is_ok());
    }
    #[test]
    fn hostile_event_counts_are_bounded() {
        assert!(parse(&json!({"method":"ui.click","params":{"count":u64::MAX}}).to_string()).is_err());
        assert!(parse(&json!({"method":"ui.drag","params":{"steps":MAX_DRAG_STEPS+1}}).to_string()).is_err());
        assert!(parse(&json!({"method":"ui.drag","params":{"steps":MAX_DRAG_STEPS}}).to_string()).is_ok());
    }
}
