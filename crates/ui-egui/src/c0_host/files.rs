//! Bytes use the engine's importers and writers; replacement uses its existing Close dialog.

use crate::SlideApp;
use serde_json::{Value, json};

pub(crate) struct Replacement {
    uid: u64,
    command: &'static str,
    params: Value,
}

pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > super::MAX_NAME_BYTES || matches!(name, "." | "..") || name.contains(['/', '\\', '\0']) {
        return Err("name must be a basename of at most 255 bytes".into());
    }
    Ok(())
}

pub fn document_bytes(app: &SlideApp, p: &Value) -> Result<Value, String> {
    let format = match p.get("format") {
        None => "pptx",
        Some(v) => v.as_str().filter(|s| matches!(*s, "pptx" | "deckcraft")).ok_or("format must be pptx or deckcraft")?,
    };
    let state = app.session.active().ok_or("no presentation open")?;
    let bytes = deckcraft_engine::cmd::file::save_bytes(&state.doc, format).map_err(|e| e.to_string())?;
    if bytes.len() > super::MAX_FILE_BYTES {
        return Err("document exceeds the host file limit".into());
    }
    let title = state.title().replace(['/', '\\', '\0'], "_");
    let mut end = title.len().min(super::MAX_NAME_BYTES.saturating_sub(format.len() + 1));
    while !title.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    let stem = title.get(..end).filter(|s| !s.is_empty()).unwrap_or("Presentation");
    Ok(json!({"name": format!("{stem}.{format}"), "format": format,
        "bytes": deckcraft_engine::cmd::base64_encode(&bytes), "revision": state.revision}))
}

pub fn open(app: &mut SlideApp, ctx: &egui::Context, p: &Value) -> Result<Value, String> {
    let name = p.get("name").and_then(Value::as_str).ok_or("missing `name`")?;
    validate_name(name)?;
    let data = p.get("bytes").and_then(Value::as_str).ok_or("missing `bytes`")?;
    if data.len() > super::MAX_FILE_BYTES.div_ceil(3) * 4 {
        return Err("file exceeds the host file limit".into());
    }
    if !padded_base64(data) {
        return Err("bytes must be standard padded base64".into());
    }
    let bytes = deckcraft_engine::cmd::base64_decode(data).ok_or("invalid base64 bytes")?;
    if bytes.len() > super::MAX_FILE_BYTES {
        return Err("file exceeds the host file limit".into());
    }
    // Validate before showing any prompt, so a bad file cannot discard good work.
    deckcraft_engine::cmd::file::open_checked(name, &bytes).map_err(|e| e.to_string())?;
    replace(app, ctx, "file.openBytes", json!({"name": name, "data": data}))
}

fn padded_base64(text: &str) -> bool {
    let sextet = |b| match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    if !text.len().is_multiple_of(4) {
        return false;
    }
    let length = text.trim_end_matches('=').len();
    let padding = text.len().saturating_sub(length);
    if padding > 2 || !text.bytes().take(length).all(|b| sextet(b).is_some()) {
        return false;
    }
    let last = length.checked_sub(1).and_then(|i| text.as_bytes().get(i)).and_then(|b| sextet(*b)).unwrap_or(0);
    match padding {
        1 => last & 3 == 0,
        2 => last & 15 == 0,
        _ => true,
    }
}

pub fn new_document(app: &mut SlideApp, ctx: &egui::Context, p: &Value) -> Result<Value, String> {
    if let Some(v) = p.get("template")
        && !v.as_str().is_some_and(|s| matches!(s, "" | "blank"))
    {
        return Err("template must be blank or omitted".into());
    }
    replace(app, ctx, "file.new", json!({}))
}

fn replace(app: &mut SlideApp, ctx: &egui::Context, command: &'static str, params: Value) -> Result<Value, String> {
    if app.host.pending.is_some() || app.dialog.is_some() || app.opening.is_some() {
        return Err("finish or cancel the current dialog or file operation first".into());
    }
    // Notes have a formatted draft until focus leaves the editor. Commit it before asking.
    if app.notes_buf.2.edited && app.session.active().is_some_and(|s| s.uid == app.notes_buf.0) {
        app.run("slide.notes", json!({"index": app.notes_buf.1, "body": app.notes_buf.2.body}))?;
        app.notes_buf.2.edited = false;
    }
    super::cancel_picker(ctx);
    app.pending_files.clear();
    if let Some(uid) = app.session.active().filter(|d| d.is_dirty()).map(|d| d.uid) {
        app.host.pending = Some(Replacement { uid, command, params });
        app.close_presentation();
        return Ok(json!({"pending": "saveChanges"}));
    }
    // Import first; if it fails the old document is still present.
    let old = app.session.active_index();
    let mut result = app.run(command, params)?;
    if let Some(index) = old {
        app.run("window.next", json!({"index":index}))?;
        app.run("file.close", json!({}))?;
        app.run("window.next", json!({"index":app.session.documents().len().saturating_sub(1)}))?;
    }
    app.backstage = None;
    result["document"] = json!(app.session.active_index());
    Ok(result)
}

pub fn tick(app: &mut SlideApp) {
    let Some(pending) = app.host.pending.as_ref() else { return };
    if !app.session.documents().iter().any(|d| d.uid == pending.uid) {
        if app.dialog.as_ref().is_some_and(|d| d.id == "close") {
            app.dialog = None;
        }
        if let Some(pending) = app.host.pending.take() {
            if let Err(error) = app.run(pending.command, pending.params) {
                app.notify(error);
            }
            app.backstage = None;
        }
    } else if app.session.active().is_none_or(|d| d.uid != pending.uid) {
        if app.dialog.as_ref().is_some_and(|d| d.id == "close") {
            app.dialog = None;
        }
        app.host.pending = None;
    } else if app.dialog.as_ref().is_none_or(|d| d.id != "close") {
        app.host.pending = None;
    }
}
