//! `c0.thumbnails`: slides as small PNGs, so the host can draw its own slide column beside the page
//! while the slide strip is hidden. Slides are drawn by the renderer the strip uses, at the strip's
//! scale. A PNG is reused while its slide and master are the same `Arc`s, the identity the strip's
//! texture cache keys on: an edited slide, or a changed master, is drawn again and nothing else is.

use std::collections::HashMap;
use std::sync::Arc;

use deckcraft_engine::cmd::base64_encode;
use deckcraft_model::{Master, Presentation, Slide};
use deckcraft_render::{MAX_SIDE, RenderOpts};
use serde_json::{Value, json};

use crate::SlideApp;
use crate::textures::fit_size;

pub const DEFAULT_COUNT: usize = 20;
pub const MAX_COUNT: usize = 50;
pub const DEFAULT_WIDTH: u32 = 160;
pub const MIN_WIDTH: u32 = 64;
pub const MAX_WIDTH: u32 = 320;
/// Cached pictures before the least recently used are dropped. A 320 px slide is tens of kilobytes.
const LIMIT: usize = 240;
const KEEP: usize = 160;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    slide: usize,
    master: usize,
    slide_size: (u64, u64),
    width: u32,
    height: u32,
}

struct Entry {
    png: String,
    width: u32,
    height: u32,
    stamp: u64,
    // Keeps the Arcs alive so their addresses cannot be reused while cached.
    _slide: Arc<Slide>,
    _master: Option<Arc<Master>>,
}

/// One thumbnail as the reply carries it: base64 PNG and its pixel size.
#[derive(Clone)]
struct Thumb {
    png: String,
    width: u32,
    height: u32,
}

#[derive(Default)]
pub(crate) struct Cache {
    entries: HashMap<Key, Entry>,
    clock: u64,
    /// PNGs drawn over the life of the cache. A repeated request for unchanged slides adds none.
    renders: u64,
}

impl Cache {
    fn thumb(&mut self, doc: &Presentation, index: usize, width: u32) -> Result<Thumb, String> {
        let slide = doc.slides.get(index).ok_or_else(|| format!("no slide {index}"))?.clone();
        let master = doc.masters.iter().find(|m| m.layout(slide.layout).is_some()).cloned();
        let (w, h) = thumb_size(doc, width);
        let key = Key {
            slide: Arc::as_ptr(&slide) as usize,
            master: master.as_ref().map_or(0, |m| Arc::as_ptr(m) as usize),
            slide_size: (doc.slide_size.width.to_bits(), doc.slide_size.height.to_bits()),
            width: w,
            height: h,
        };
        self.clock += 1;
        let stamp = self.clock;
        if let Some(e) = self.entries.get_mut(&key) {
            e.stamp = stamp;
            return Ok(Thumb { png: e.png.clone(), width: e.width, height: e.height });
        }
        let opts = RenderOpts { scale: f64::from(w) / doc.slide_size.width.max(1.0), size: Some((w, h)), threads: 0, ..Default::default() };
        let img = deckcraft_render::render_slide(doc, index, &opts);
        let png = img.to_png();
        if png.is_empty() {
            return Err(format!("slide {index} could not be encoded as PNG"));
        }
        self.renders += 1;
        let thumb = Thumb { png: base64_encode(&png), width: img.width, height: img.height };
        self.entries.insert(
            key,
            Entry { png: thumb.png.clone(), width: thumb.width, height: thumb.height, stamp, _slide: slide, _master: master },
        );
        self.trim();
        Ok(thumb)
    }

    /// Drops the least recently used pictures once the cache passes its limit.
    fn trim(&mut self) {
        if self.entries.len() <= LIMIT {
            return;
        }
        let mut stamps: Vec<u64> = self.entries.values().map(|e| e.stamp).collect();
        stamps.sort_unstable();
        if let Some(&cut) = stamps.get(stamps.len().saturating_sub(KEEP)) {
            self.entries.retain(|_, e| e.stamp >= cut);
        }
    }
}

/// The picture's pixel size: `width` wide, the slide's shape kept. A slide taller than the
/// renderer can draw is shrunk to fit, so its width can come out narrower than asked.
fn thumb_size(doc: &Presentation, width: u32) -> (u32, u32) {
    let (sw, sh) = (doc.slide_size.width.max(1.0), doc.slide_size.height.max(1.0));
    let h = (f64::from(width) * sh / sw).round().max(1.0) as u32;
    fit_size((width, h), MAX_SIDE)
}

/// A whole-number parameter, or `None` when absent or null.
fn whole(p: &Value, key: &str) -> Result<Option<u64>, String> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().map(Some).ok_or_else(|| format!("{key} must be a whole number")),
    }
}

pub(crate) fn read(app: &mut SlideApp, p: &Value) -> Result<Value, String> {
    let from = whole(p, "from")?.map_or(0, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let count = match whole(p, "count")? {
        None => DEFAULT_COUNT,
        Some(0) => return Err("count must be at least 1".into()),
        Some(n) => usize::try_from(n).unwrap_or(MAX_COUNT).min(MAX_COUNT),
    };
    let width = match p.get("width") {
        None | Some(Value::Null) => DEFAULT_WIDTH,
        Some(v) => {
            let w = v.as_f64().filter(|w| w.is_finite()).ok_or("width must be a number of pixels")?;
            w.round().clamp(f64::from(MIN_WIDTH), f64::from(MAX_WIDTH)) as u32
        }
    };
    let Some(doc) = app.session.active().map(|s| s.doc.clone()) else {
        return Ok(json!({"slides": [], "total": 0}));
    };
    let total = doc.slides.len();
    let start = from.min(total);
    let end = start.saturating_add(count).min(total);
    let mut slides = Vec::with_capacity(end - start);
    for index in start..end {
        let thumb = app.host.thumbs.thumb(&doc, index, width)?;
        let id = doc.slides.get(index).map(|s| s.id);
        slides.push(json!({"index": index, "id": id, "png": thumb.png, "width": thumb.width, "height": thumb.height}));
    }
    Ok(json!({"slides": slides, "total": total}))
}

#[cfg(test)]
mod tests {
    use super::read;
    use crate::{Services, SlideApp};
    use deckcraft_engine::Session;
    use serde_json::json;

    #[test]
    fn unchanged_slides_are_not_drawn_again() {
        let mut app = SlideApp::new(Session::new(), Services::default());
        app.run("file.new", json!({})).unwrap();
        app.run("slide.new", json!({})).unwrap();
        read(&mut app, &json!({"width": 100})).unwrap();
        assert_eq!(app.host.thumbs.renders, 2);
        read(&mut app, &json!({"width": 100})).unwrap();
        assert_eq!(app.host.thumbs.renders, 2, "a repeated request draws nothing");
        app.run("slide.hide", json!({"index": 1, "hidden": true})).unwrap();
        read(&mut app, &json!({"width": 100})).unwrap();
        assert_eq!(app.host.thumbs.renders, 3, "only the edited slide is drawn again");
        read(&mut app, &json!({"width": 120})).unwrap();
        assert_eq!(app.host.thumbs.renders, 5, "another width is another picture");
    }
}
