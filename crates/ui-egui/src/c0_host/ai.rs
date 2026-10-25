//! `c0.selection` and `c0.place`: what the C0VIBE AI actions read from the deck and put back.
//!
//! Not upstream's: part of the C0VIBE host shell (`c0/deckcraft-host/README.md`). Every edit goes
//! through the engine's own commands, so history, dirty state and saving behave as for a user edit,
//! and a whole placement is ONE undo step ([`one_step`]).
//!
//! | Method | Params | Result |
//! |---|---|---|
//! | `c0.selection` | `{}` | `{kind:"slide", slideIndex, slideCount, title, bodyText, notes, selectedText?, outline:[{index,title}]}` |
//! | `c0.place` | exactly one of `{slides:[{title,bullets:[string],notes?}], after?}`, `{text, mode:"replace-selection"}`, `{notes, slideIndex?}`, `{png\|jpeg, name}` | `{placed:"slides"\|"text"\|"notes"\|"picture", ...}` |

use serde_json::{Value, json};

use crate::SlideApp;

/// The biggest picture a request may carry, decoded.
pub const MAX_PICTURE_BYTES: usize = 40 * 1024 * 1024;
const MAX_SLIDES: usize = 100;
const MAX_BULLETS: usize = 200;
const MAX_TEXT_CHARS: usize = 200_000;

pub fn methods() -> [&'static str; 2] {
    ["c0.selection", "c0.place"]
}

// ---------- c0.selection ----------

fn range_text(body: &deckcraft_model::TextBody, a: (usize, usize), b: (usize, usize)) -> String {
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    let mut out = String::new();
    for pi in a.0..=b.0.min(body.paragraphs.len().saturating_sub(1)) {
        let Some(p) = body.paragraphs.get(pi) else { break };
        let text = p.text();
        let from = if pi == a.0 { a.1 } else { 0 };
        let to = if pi == b.0 { b.1 } else { usize::MAX };
        if pi > a.0 {
            out.push('\n');
        }
        out.extend(text.chars().skip(from).take(to.saturating_sub(from)));
    }
    out
}

fn selected_text(st: &deckcraft_engine::DocState) -> Option<String> {
    let slide = st.doc.slides.get(st.selection.slide)?;
    if let Some(t) = st.selection.text.as_ref().filter(|t| t.is_range() && t.cell.is_none()) {
        let body = if t.notes { Some(&slide.notes) } else { st.shape(t.shape).and_then(|s| s.text.as_ref()) };
        return body.map(|b| range_text(b, t.anchor, t.caret)).filter(|s| !s.is_empty());
    }
    if st.selection.text.is_none() && st.selection.shapes.len() == 1 {
        let text = st.shape(st.selection.shapes[0])?.text.as_ref()?.text();
        return Some(text).filter(|s| !s.trim().is_empty());
    }
    None
}

pub fn selection(app: &SlideApp) -> Result<Value, String> {
    let st = app.session.active().ok_or("no presentation open")?;
    let slide = st.doc.slides.get(st.selection.slide).ok_or("the presentation has no slides")?;
    let body: Vec<String> = slide
        .shapes
        .iter()
        .filter(|s| !s.ph_type().is_some_and(|k| k.is_title()))
        .filter_map(|s| s.text.as_ref())
        .map(|t| t.text().replace('\u{b}', "\n"))
        .filter(|t| !t.trim().is_empty())
        .collect();
    let outline: Vec<Value> = st.doc.slides.iter().enumerate().map(|(i, s)| json!({"index": i, "title": s.title()})).collect();
    let mut out = json!({
        "kind": "slide", "slideIndex": st.selection.slide, "slideCount": st.doc.slides.len(),
        "title": slide.title(), "bodyText": body.join("\n"), "notes": slide.notes_text(), "outline": outline,
    });
    if let Some(text) = selected_text(st) {
        out["selectedText"] = json!(text);
    }
    Ok(out)
}

// ---------- one undo step ----------

/// Runs `f` and makes everything it did one undo step; if it fails, the deck, selection and history
/// are exactly as they were.
fn one_step<T>(app: &mut SlideApp, label: &str, f: impl FnOnce(&mut SlideApp) -> Result<T, String>) -> Result<T, String> {
    let st = app.session.active().ok_or("no presentation open")?;
    let (doc, selection, history) = (st.doc.clone(), st.selection.clone(), st.history.clone());
    // The Edit Points type remembered with the old deck, as the engine's own undo step keeps it.
    let vertex_kind = app.session.tool.vertex_kind.clone();
    let result = f(app);
    if result.is_err() {
        app.session.tool.vertex_kind = vertex_kind.clone();
    }
    let Some(st) = app.session.active_mut() else { return result };
    match result {
        Ok(v) => {
            if !std::sync::Arc::ptr_eq(&doc, &st.doc) {
                st.history = history;
                let h = &mut st.history;
                h.costs.insert(std::sync::Arc::as_ptr(&doc) as usize, deckcraft_engine::undo_cost(&doc, &st.doc));
                h.undo.push(deckcraft_engine::HistoryEntry { label: label.into(), doc, selection, vertex_kind });
                h.redo.clear();
                if h.undo.len() > h.limit.max(1) {
                    h.undo.remove(0);
                }
            }
            Ok(v)
        }
        Err(e) => {
            st.doc = doc;
            st.selection = selection;
            st.history = history;
            st.composition = None;
            st.revision += 1;
            Err(e)
        }
    }
}

// ---------- c0.place ----------

fn text_param(p: &Value, key: &str) -> Result<String, String> {
    match p.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() && s.chars().count() <= MAX_TEXT_CHARS => Ok(s.clone()),
        Some(Value::String(s)) if s.trim().is_empty() => Err(format!("`{key}` is empty")),
        Some(Value::String(_)) => Err(format!("`{key}` is longer than {MAX_TEXT_CHARS} characters")),
        _ => Err(format!("`{key}` must be text")),
    }
}

fn index_param(p: &Value, key: &str, count: usize) -> Result<Option<usize>, String> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => match v.as_u64().and_then(|n| usize::try_from(n).ok()) {
            Some(n) if n < count => Ok(Some(n)),
            _ => Err(format!("`{key}` must be the index of an existing slide (0 to {})", count.saturating_sub(1))),
        },
    }
}

pub fn place(app: &mut SlideApp, p: &Value) -> Result<Value, String> {
    let kinds: Vec<&str> = ["slides", "text", "notes", "png", "jpeg"].into_iter().filter(|k| p.get(*k).is_some_and(|v| !v.is_null())).collect();
    match kinds.as_slice() {
        ["slides"] => place_slides(app, p),
        ["text"] => place_text(app, p),
        ["notes"] => place_notes(app, p),
        ["png"] | ["jpeg"] => place_picture(app, p, kinds[0]),
        [] => Err("nothing to place: give `slides`, `text`, `notes`, `png` or `jpeg`".into()),
        _ => Err("place one thing at a time: give only one of `slides`, `text`, `notes`, `png` or `jpeg`".into()),
    }
}

fn place_slides(app: &mut SlideApp, p: &Value) -> Result<Value, String> {
    let items = p["slides"].as_array().ok_or("`slides` must be a list")?;
    if items.is_empty() || items.len() > MAX_SLIDES {
        return Err(format!("`slides` must hold 1 to {MAX_SLIDES} slides"));
    }
    let mut parsed = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let title = match item.get("title") {
            Some(Value::String(t)) => t.clone(),
            _ => return Err(format!("slides[{i}].title must be text")),
        };
        let bullets = match item.get("bullets") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(b)) if b.len() <= MAX_BULLETS => b
                .iter()
                .map(|x| x.as_str().map(str::to_owned).ok_or(format!("slides[{i}].bullets must be a list of text")))
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(format!("slides[{i}].bullets must be a list of at most {MAX_BULLETS} texts")),
        };
        let notes = match item.get("notes") {
            None | Some(Value::Null) => None,
            Some(Value::String(n)) => Some(n.clone()),
            Some(_) => return Err(format!("slides[{i}].notes must be text")),
        };
        if title.chars().count() + bullets.iter().map(|b| b.chars().count()).sum::<usize>() > MAX_TEXT_CHARS {
            return Err(format!("slides[{i}] is too long"));
        }
        parsed.push((title, bullets, notes));
    }
    let (count, current) = {
        let st = app.session.active().ok_or("no presentation open")?;
        (st.doc.slides.len(), st.selection.slide)
    };
    let after = index_param(p, "after", count)?;
    let first = if count == 0 { 0 } else { after.unwrap_or(current) + 1 };
    one_step(app, "Add Slides", |app| {
        let mut indices = Vec::new();
        for (n, (title, bullets, notes)) in parsed.iter().enumerate() {
            let at = first + n;
            let mut params = json!({"layout": "titleAndContent", "title": title, "at": at});
            if !bullets.is_empty() {
                params["body"] = json!(bullets.join("\n"));
            }
            let made = app.run("slide.new", params)?;
            let index = made["index"].as_u64().map_or(at, |i| i as usize);
            if let Some(notes) = notes.as_deref().filter(|n| !n.trim().is_empty()) {
                app.run("slide.notes", json!({"text": notes, "index": index}))?;
            }
            indices.push(index);
        }
        Ok(json!({"placed": "slides", "count": indices.len(), "indices": indices}))
    })
}

fn place_text(app: &mut SlideApp, p: &Value) -> Result<Value, String> {
    if p.get("mode").and_then(Value::as_str) != Some("replace-selection") {
        return Err("`mode` must be \"replace-selection\"".into());
    }
    let text = text_param(p, "text")?;
    let (range, shape) = {
        let st = app.session.active().ok_or("no presentation open")?;
        let range = st.selection.text.as_ref().is_some_and(|t| t.is_range());
        let shape = (st.selection.text.is_none() && st.selection.shapes.len() == 1 && st.shape(st.selection.shapes[0]).is_some_and(|s| s.text.is_some()))
            .then(|| st.selection.shapes[0]);
        (range, shape)
    };
    if !range && shape.is_none() {
        return Err("nothing to replace: select some text or a text box first".into());
    }
    one_step(app, "Replace Text", |app| {
        if let Some(id) = shape {
            app.run("text.edit", json!({"id": id}))?;
            app.run("text.selectAll", json!({}))?;
        }
        app.run("text.insert", json!({"text": text}))?;
        if shape.is_some() {
            app.run("text.exit", json!({}))?;
        }
        Ok(json!({"placed": "text", "characters": text.chars().count()}))
    })
}

fn place_notes(app: &mut SlideApp, p: &Value) -> Result<Value, String> {
    let notes = text_param(p, "notes")?;
    let (count, current) = {
        let st = app.session.active().ok_or("no presentation open")?;
        (st.doc.slides.len(), st.selection.slide)
    };
    if count == 0 {
        return Err("the presentation has no slides".into());
    }
    let index = index_param(p, "slideIndex", count)?.unwrap_or(current);
    one_step(app, "Notes", |app| {
        app.run("slide.notes", json!({"text": notes, "index": index}))?;
        Ok(json!({"placed": "notes", "slideIndex": index}))
    })
}

/// Width and height in pixels of a PNG or JPEG, from its header.
fn picture_size(b: &[u8], png: bool) -> Option<(u32, u32)> {
    if png {
        if b.len() < 24 || b[..8] != [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] {
            return None;
        }
        let n = |o: usize| u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        return Some((n(16), n(20))).filter(|d| d.0 > 0 && d.1 > 0);
    }
    if b.len() < 4 || b[..2] != [0xff, 0xd8] {
        return None;
    }
    let mut i = 2;
    while i + 9 < b.len() {
        if b[i] != 0xff {
            i += 1;
            continue;
        }
        let m = b[i + 1];
        if m == 0xff || m == 0x01 || (0xd0..=0xd8).contains(&m) {
            i += if m == 0xff { 1 } else { 2 };
            continue;
        }
        let len = usize::from(u16::from_be_bytes([b[i + 2], b[i + 3]]));
        if (0xc0..=0xcf).contains(&m) && !matches!(m, 0xc4 | 0xc8 | 0xcc) {
            let h = u32::from(u16::from_be_bytes([b[i + 5], b[i + 6]]));
            let w = u32::from(u16::from_be_bytes([b[i + 7], b[i + 8]]));
            return Some((w, h)).filter(|d| d.0 > 0 && d.1 > 0);
        }
        i += 2 + len.max(2);
    }
    None
}

fn place_picture(app: &mut SlideApp, p: &Value, field: &str) -> Result<Value, String> {
    let png = field == "png";
    let data = p[field].as_str().ok_or_else(|| format!("`{field}` must be base64 text"))?;
    if data.len() > MAX_PICTURE_BYTES / 3 * 4 + 1024 {
        return Err("the picture is larger than 40 MB".into());
    }
    let bytes = deckcraft_engine::cmd::base64_decode(data).filter(|b| !b.is_empty()).ok_or("`png`/`jpeg` is not valid base64")?;
    if bytes.len() > MAX_PICTURE_BYTES {
        return Err("the picture is larger than 40 MB".into());
    }
    let (w, h) = picture_size(&bytes, png).ok_or_else(|| format!("the data is not a readable {}", if png { "PNG" } else { "JPEG" }))?;
    let name = match p.get("name") {
        Some(Value::String(n)) if !n.trim().is_empty() && n.len() <= super::MAX_NAME_BYTES && !n.contains(['/', '\\', '\0']) => n.trim().to_owned(),
        _ => return Err("`name` must be a short file name".into()),
    };
    let ext = if png { ".png" } else { ".jpg" };
    let name = if name.to_lowercase().ends_with(ext) || (!png && name.to_lowercase().ends_with(".jpeg")) { name } else { format!("{name}{ext}") };
    let size = app.session.active().ok_or("no presentation open")?.doc.slide_size;
    if app.session.active().is_some_and(|s| s.doc.slides.is_empty()) {
        return Err("the presentation has no slides".into());
    }
    let k = (size.width / f64::from(w)).min(size.height / f64::from(h));
    let (pw, ph) = (f64::from(w) * k, f64::from(h) * k);
    let rect = [(size.width - pw) / 2.0, (size.height - ph) / 2.0, pw, ph];
    let data = data.to_owned();
    one_step(app, "Insert Picture", |app| {
        // Nothing selected, so the picture is not poured into an empty placeholder.
        app.session.select(|_, sel| {
            sel.shapes.clear();
            sel.text = None;
        }).map_err(|e| e.to_string())?;
        let made = app.run("insert.picture", json!({"name": name, "data": data, "rect": rect}))?;
        Ok(json!({"placed": "picture", "id": made["id"], "slideIndex": app.session.active().map(|s| s.selection.slide),
            "rect": {"x": rect[0], "y": rect[1], "width": rect[2], "height": rect[3]}}))
    })
}
