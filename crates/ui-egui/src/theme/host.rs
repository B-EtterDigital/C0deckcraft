//! Runtime, context-local palette. Colours replace the override set and survive brightness changes.

use super::Tokens;
use egui::Color32;
use serde_json::{Map, Value};
use std::sync::Arc;

#[derive(Default)]
pub struct Report {
    pub applied: Vec<String>,
    pub unknown: Vec<String>,
    pub invalid: Vec<String>,
}

fn key() -> egui::Id {
    egui::Id::new("c0deck-theme")
}

pub fn set(ctx: &egui::Context, value: &Value) -> Result<Report, String> {
    let entries = match value {
        Value::Null => None,
        Value::Object(entries) => Some(entries),
        _ => return Err("themeTokens must be an object or null".into()),
    };
    let mut report = Report::default();
    let mut clean = Map::new();
    for (name, value) in entries.into_iter().flatten() {
        if !assign(&mut Tokens::light(), name, Color32::TRANSPARENT) {
            report.unknown.push(name.clone());
        } else if colour(value).is_some() {
            clean.insert(name.clone(), value.clone());
            report.applied.push(name.clone());
        } else {
            report.invalid.push(name.clone());
        }
    }
    ctx.data_mut(|d| {
        d.insert_temp(key(), Arc::new(Value::Object(clean)));
    });
    super::apply(ctx, &resolve(ctx, Tokens::get(ctx)));
    Ok(report)
}

pub(super) fn resolve(ctx: &egui::Context, mut tokens: Tokens) -> Tokens {
    if let Some(values) = ctx.data(|d| d.get_temp::<Arc<Value>>(key())) {
        for (name, value) in values.as_object().into_iter().flatten() {
            if let Some(color) = colour(value) {
                assign(&mut tokens, name, color);
            }
        }
    }
    tokens
}

fn colour(value: &Value) -> Option<Color32> {
    let hex = value.as_str()?.strip_prefix('#')?;
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |at| hex.get(at..at + 2).and_then(|s| u8::from_str_radix(s, 16).ok());
    Some(Color32::from_rgba_unmultiplied(byte(0)?, byte(2)?, byte(4)?, if hex.len() == 8 { byte(6)? } else { 255 }))
}

fn assign(t: &mut Tokens, key: &str, color: Color32) -> bool {
    macro_rules! colors { ($($field:ident),* $(,)?) => { match key { $(stringify!($field) => { t.$field = color; true },)* _ => false } }; }
    colors!(
        chrome,
        card,
        card_border,
        canvas,
        text,
        text_dim,
        text_faint,
        border,
        separator,
        hover,
        pressed,
        selected,
        accent,
        accent_text,
        contextual,
        selection,
        handle_fill,
        handle_stroke,
        guide,
        caret,
        text_selection,
        field_bg,
        shadow
    )
}
