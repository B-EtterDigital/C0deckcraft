use crate::{SlideApp, theme};
use serde_json::{Value, json};

pub fn set(app: &mut SlideApp, ctx: &egui::Context, p: &Value) -> Result<Value, String> {
    if p.get("panels").is_some_and(|v| !v.is_object()) {
        return Err("panels must be an object".into());
    }
    if p.get("themeTokens").is_some_and(|v| !v.is_object() && !v.is_null()) {
        return Err("themeTokens must be an object or null".into());
    }
    let mut applied = vec![];
    let mut unknown = vec![];
    let mut invalid = vec![];
    for key in p.as_object().into_iter().flatten().map(|(k, _)| k) {
        if !matches!(key.as_str(), "panels" | "themeTokens") {
            unknown.push(key.clone());
        }
    }
    if let Some(tokens) = p.get("themeTokens") {
        let report = theme::host::set(ctx, tokens)?;
        applied.extend(report.applied.into_iter().map(|k| format!("themeTokens.{k}")));
        unknown.extend(report.unknown.into_iter().map(|k| format!("themeTokens.{k}")));
        invalid.extend(report.invalid.into_iter().map(|k| format!("themeTokens.{k}")));
    }
    for (key, value) in p.get("panels").and_then(Value::as_object).into_iter().flatten() {
        let name = format!("panels.{key}");
        let slot = match key.as_str() {
            "title_bar" => &mut app.chrome.title_bar,
            "file_tab" => &mut app.chrome.file_tab,
            "start_screen" => &mut app.chrome.start_screen,
            "community_links" => &mut app.chrome.community_links,
            "status_bar" => &mut app.chrome.status_bar,
            "ribbon_collapsed" => &mut app.ui.ribbon_collapsed,
            "notes" => &mut app.ui.notes,
            "ruler" => &mut app.ui.ruler,
            _ => {
                unknown.push(name);
                continue;
            }
        };
        match value.as_bool() {
            Some(value) => {
                *slot = value;
                applied.push(name);
            }
            None => invalid.push(name),
        }
    }
    if !app.chrome.file_tab || (!app.chrome.start_screen && app.session.active().is_none()) {
        app.backstage = None;
    }
    if !app.chrome.community_links {
        use deckcraft_engine::links::*;
        app.pending_urls.retain(|url| ![DISCORD, WEBSITE, APPS, APP_PAGE, GITHUB].contains(&url.as_str()));
    }
    ctx.data_mut(|d| d.insert_temp(super::community_key(), app.chrome.community_links));
    theme::apply(ctx, &theme::Tokens::get(ctx));
    ctx.request_repaint();
    Ok(json!({"applied": applied, "unknown": unknown, "invalid": invalid}))
}
