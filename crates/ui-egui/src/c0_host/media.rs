//! Page builds reject explicit playback of imported restricted media before queuing playback.

use crate::SlideApp;
use deckcraft_model::{ShapeId, ShapeKind};
use serde_json::Value;

pub(crate) fn preflight_media(app: &mut SlideApp, command: &str, params: &Value) -> Result<(), String> {
    if deckcraft_media::c0_codecs::enabled("H.264") || !matches!(command, "media.play" | "media.toggle") {
        return Ok(());
    }
    let result = check(app, command, params);
    if let Err(error) = &result {
        app.set_status(error.clone());
    }
    result
}

fn check(app: &mut SlideApp, command: &str, params: &Value) -> Result<(), String> {
    // Resolve targets through the real engine command (selection, explicit id and single-media
    // fallback), so this path cannot diverge from media.play as upstream adds target semantics.
    let info = app.session.execute("media.info", params).map_err(|e| e.to_string())?;
    if command == "media.toggle" && info.get("status").and_then(|s| s.get("state")).and_then(Value::as_str) == Some("playing") {
        return Ok(()); // Pausing must remain possible, including after an autoplay error.
    }
    let id = info.get("id").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).ok_or("media has no shape id")?;
    let state = app.session.active().ok_or("no presentation open")?;
    let Some(ShapeKind::Media(clip)) = state.shape(ShapeId(id)).map(|s| &s.kind) else {
        return Err("not a media shape".into());
    };
    let item = state.doc.media(clip.media).ok_or("the media data is missing")?;
    deckcraft_media::c0_codecs::check(&item.data).map_err(|e| e.to_string())
}
