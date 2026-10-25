//! C0deck's transport-independent host contract. Host state is never persisted in a deck.

mod ai;
mod chrome;
#[cfg(target_arch = "wasm32")]
mod drops;
mod files;
pub use files::validate_name;
pub use ai::{MAX_PICTURE_BYTES, methods as ai_methods};
mod settings;
mod tools;
#[cfg(target_arch = "wasm32")]
pub use drops::{FileReadBudget, read_drop, reserve_file_bytes};
mod media;
pub(crate) use chrome::{about_links, account_links, community_link, learn_more, palette, status_bar, title_bar};
pub(crate) use media::preflight_media;
pub(crate) use tools::bars;

use std::sync::{Arc, atomic::AtomicBool};

use serde_json::{Value, json};

use crate::{SlideApp, theme};

pub const MAX_FILE_BYTES: usize = 72 * 1024 * 1024;
pub const MAX_REQUEST_BYTES: usize = 100 * 1024 * 1024;
pub const MAX_NAME_BYTES: usize = 255;

/// All defaults preserve the standalone application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chrome {
    pub title_bar: bool,
    pub file_tab: bool,
    pub start_screen: bool,
    pub community_links: bool,
    pub status_bar: bool,
}

impl Default for Chrome {
    fn default() -> Self {
        Self { title_bar: true, file_tab: true, start_screen: true, community_links: true, status_bar: true }
    }
}

#[derive(Default)]
pub(crate) struct State {
    pending: Option<files::Replacement>,
    pub scroll_to: Option<usize>,
    pub(crate) tools: tools::Tools,
}

pub fn handle(app: &mut SlideApp, ctx: &egui::Context, method: &str, params: &Value) -> Result<Value, String> {
    if !params.is_object() {
        return Err("params must be an object".into());
    }
    match method {
        "c0.inspect" => Ok(inspect(app)),
        "c0.goTo" => go_to(app, params),
        "c0.documentBytes" => files::document_bytes(app, params),
        "c0.openFile" => files::open(app, ctx, params),
        "c0.newDocument" => files::new_document(app, ctx, params),
        "c0.cancelFileDialog" => {
            let picker = cancel_picker(ctx);
            let pending = !app.pending_files.is_empty();
            app.pending_files.clear();
            Ok(json!({"cancelled": picker || pending}))
        }
        "c0.show" => show(app, params),
        "c0.set" => settings::set(app, ctx, params),
        "c0.selection" => ai::selection(app),
        "c0.place" => ai::place(app, params),
        _ => Err(format!("unknown method `{method}`")),
    }
}

pub fn inspect(app: &SlideApp) -> Value {
    let state = app.session.active();
    let slides: Vec<Value> = state
        .into_iter()
        .flat_map(|s| {
            s.doc.slides.iter().enumerate().map(move |(i, sl)| {
                let mut v = json!({"index": i, "id": sl.id, "title": sl.title(), "hidden": sl.hidden});
                if let Some(section) = s.doc.section_of(sl.id).and_then(|i| s.doc.sections.get(i)) {
                    v["section"] = json!(section.name);
                }
                v
            })
        })
        .collect();
    let sections: Vec<Value> = state
        .into_iter()
        .flat_map(|s| {
            s.doc.sections.iter().map(|section| {
                let indices: Vec<_> = section.slides.iter().filter_map(|id| s.doc.slide_index(*id)).collect();
                json!({"name": section.name, "first": indices.iter().min(), "count": indices.len()})
            })
        })
        .collect();
    let mut panes = json!({"notes": app.ui.notes, "ruler": app.ui.ruler, "thumbnails": state.is_some() && matches!(app.ui.view, crate::ViewMode::Normal | crate::ViewMode::Reading)});
    tools::report(app, &mut panes);
    json!({
        "title": state.map(|s| s.title()), "dirty": state.is_some_and(|s| s.is_dirty()),
        "revision": state.map_or(0, |s| s.revision), "slideCount": slides.len(), "slides": slides,
        "currentSlide": state.filter(|s| !s.doc.slides.is_empty()).map(|s| s.selection.slide), "sections": sections,
        "canUndo": state.is_some_and(|s| !s.history.undo.is_empty()), "canRedo": state.is_some_and(|s| !s.history.redo.is_empty()),
        "panes": panes, "toolbarFloat": tools::float_pos(app),
        "ribbonCollapsed": app.ui.ribbon_collapsed, "showing": app.show.is_some(),
        "theme": match app.ui.brightness { theme::Brightness::Light => "light", theme::Brightness::Dark => "dark" }
    })
}

fn go_to(app: &mut SlideApp, p: &Value) -> Result<Value, String> {
    let slide = p.get("slide").ok_or("missing `slide`")?;
    // Prefer a valid index; an out-of-range numeric id is useful directly from inspect.
    // String ids (or {id:number}) disambiguate an id that is also a valid index.
    let params = if let Some(index) = slide.as_u64() {
        let numeric_id = app.session.active().is_some_and(|s| {
            usize::try_from(index).ok().is_none_or(|i| i >= s.doc.slides.len()) && s.doc.slides.iter().any(|sl| u64::from(sl.id.0) == index)
        });
        if numeric_id { json!({"id": index}) } else { json!({"index": index}) }
    } else if let Some(id) = slide.as_str().and_then(|s| s.parse::<u32>().ok()).map(u64::from).or_else(|| slide.get("id").and_then(Value::as_u64)) {
        json!({"id": id})
    } else {
        return Err("slide must be a zero-based index, a string id, or {id:number}".into());
    };
    if params.get("index").and_then(Value::as_u64).is_some_and(|n| usize::try_from(n).is_err()) {
        return Err("slide index is out of range".into());
    }
    let result = app.run("slide.go", params)?;
    app.host.scroll_to = app.session.active().map(|s| s.selection.slide);
    Ok(result)
}

fn show(app: &mut SlideApp, p: &Value) -> Result<Value, String> {
    match p.get("action").and_then(Value::as_str) {
        Some("start") => {
            let state = app.session.active().ok_or("no presentation open")?;
            let from = match p.get("from") {
                None => app.show_start_index(),
                Some(v) => v.as_u64().and_then(|v| usize::try_from(v).ok()).ok_or("from must be a zero-based slide index")?,
            };
            if from >= state.doc.slides.len() {
                return Err("no such slide".into());
            }
            app.start_show(from, true);
        }
        Some("end") => {
            app.run("show.end", json!({}))?;
        }
        _ => return Err("action must be start or end".into()),
    }
    Ok(json!({"showing": app.show.is_some()}))
}

fn picker_key() -> egui::Id {
    egui::Id::new("c0deck-picker")
}

pub(super) fn community_key() -> egui::Id {
    egui::Id::new("c0deck-community")
}

pub fn begin_picker(ctx: &egui::Context) -> Arc<AtomicBool> {
    cancel_picker(ctx);
    let pending = Arc::new(AtomicBool::new(true));
    ctx.data_mut(|d| d.insert_temp(picker_key(), pending.clone()));
    pending
}

pub fn cancel_picker(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<Arc<AtomicBool>>(picker_key())).is_some_and(|p| p.swap(false, std::sync::atomic::Ordering::SeqCst))
}

/// Called before the control queue: a successful existing Close dialog completes replacement.
pub(crate) fn tick(app: &mut SlideApp, ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(community_key(), app.chrome.community_links));
    files::tick(app);
}

/// Contributor names stay visible; the host can remove their outbound GitHub links too.
pub(crate) fn credit(ui: &mut egui::Ui, name: String, url: String) -> egui::Response {
    if ui.ctx().data(|d| d.get_temp::<bool>(community_key())).unwrap_or(true) { ui.hyperlink_to(name, url) } else { ui.label(name) }
}

pub(crate) fn scroll(app: &mut SlideApp, ui: &egui::Ui, index: usize, row: egui::Rect) {
    if app.host.scroll_to == Some(index) {
        ui.scroll_to_rect(row, Some(egui::Align::Center));
        app.host.scroll_to = None;
    }
}
