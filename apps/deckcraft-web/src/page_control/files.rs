//! Native browser chooser without rfd's HTML overlay, with bounded reads and retired answers.

use deckcraft_ui_egui::{Inbox, InboxEvent, InboxFile, SlideApp, c0_host};
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use wasm_bindgen::{JsCast as _, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};

thread_local! { static PICKER: RefCell<Option<Picker>> = const { RefCell::new(None) }; }

struct Picker {
    input: web_sys::HtmlInputElement,
    pending: Arc<AtomicBool>,
    // Own listeners until the next request; never drop a closure during its callback.
    _change: Closure<dyn FnMut(web_sys::Event)>,
    _cancel: Closure<dyn FnMut(web_sys::Event)>,
    id: u64,
    inbox: Inbox,
    ctx: egui::Context,
}

impl Drop for Picker {
    fn drop(&mut self) {
        self.pending.store(false, Ordering::SeqCst);
        self.input.set_onchange(None);
        let _ = self.input.remove_event_listener_with_callback("cancel", self._cancel.as_ref().unchecked_ref());
        self.input.remove();
        push(&self.inbox, InboxEvent::Cancelled { id: self.id });
        self.ctx.request_repaint();
    }
}

pub fn retire() {
    PICKER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|p| !p.pending.load(Ordering::SeqCst))
            && let Some(picker) = slot.take()
        {
            picker.input.remove();
        }
    });
}

fn held(inbox: &Inbox) -> usize {
    inbox
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .map(|event| match event {
            InboxEvent::File(file) => file.bytes.len(),
            _ => 0,
        })
        .fold(0, usize::saturating_add)
}

fn finish(inbox: &Inbox, ctx: &egui::Context, pending: &AtomicBool, event: InboxEvent) {
    if pending.swap(false, Ordering::SeqCst) {
        push(inbox, event);
        ctx.request_repaint();
    }
}

fn push(inbox: &Inbox, event: InboxEvent) {
    inbox.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(event);
}

pub fn attach(app: &mut SlideApp, ctx: &egui::Context, filters: [&'static [&'static str]; 4]) {
    let inbox = app.services.inbox.clone();
    let ctx = ctx.clone();
    app.services.open_async = Some(Box::new(move |purpose, id| {
        let Some(inbox) = inbox.clone() else { return };
        let pending = c0_host::begin_picker(&ctx);
        retire();
        let exts = match purpose {
            "picture" => filters[1],
            "audio" => filters[2],
            "video" => filters[3],
            _ => filters[0],
        };
        match picker(exts, id, inbox.clone(), ctx.clone(), pending.clone()) {
            Ok(picker) => {
                picker.input.click();
                PICKER.with(|slot| *slot.borrow_mut() = Some(picker));
            }
            Err(error) => {
                log::warn!("page-control: cannot open file chooser: {error}");
                finish(&inbox, &ctx, &pending, InboxEvent::Cancelled { id });
            }
        }
    }));
}

fn picker(exts: &[&str], id: u64, inbox: Inbox, ctx: egui::Context, pending: Arc<AtomicBool>) -> Result<Picker, String> {
    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let input: web_sys::HtmlInputElement = document.create_element("input").map_err(|e| format!("{e:?}"))?.dyn_into().map_err(|_| "not an input")?;
    input.set_type("file");
    input.set_id("c0deck-file-picker");
    input.set_hidden(true);
    input.set_accept(&exts.iter().map(|ext| format!(".{ext}")).collect::<Vec<_>>().join(","));
    document.body().ok_or("no body")?.append_child(&input).map_err(|e| format!("{e:?}"))?;
    let selected = input.clone();
    let open = pending.clone();
    let open_inbox = inbox.clone();
    let open_ctx = ctx.clone();
    let change = Closure::new(move |_: web_sys::Event| {
        if !open.load(Ordering::SeqCst) {
            return;
        }
        let Some(file) = selected.files().and_then(|f| f.get(0)) else {
            finish(&open_inbox, &open_ctx, &open, InboxEvent::Cancelled { id });
            selected.remove();
            return;
        };
        let open = open.clone();
        let inbox = open_inbox.clone();
        let ctx = open_ctx.clone();
        let selected = selected.clone();
        spawn_local(async move {
            let result = read(&file, &inbox, &open).await;
            let event = match result {
                Ok(bytes) => InboxEvent::File(InboxFile { id, name: file.name(), bytes }),
                Err(error) => {
                    log::warn!("page-control: {error}");
                    InboxEvent::Cancelled { id }
                }
            };
            finish(&inbox, &ctx, &open, event);
            selected.remove();
        });
    });
    let cancel_input = input.clone();
    let cancelled = pending.clone();
    let cancel_inbox = inbox.clone();
    let cancel_ctx = ctx.clone();
    let cancel = Closure::new(move |_: web_sys::Event| {
        finish(&cancel_inbox, &cancel_ctx, &cancelled, InboxEvent::Cancelled { id });
        cancel_input.remove();
    });
    let picker = Picker { input, pending, _change: change, _cancel: cancel, id, inbox, ctx };
    picker.input.set_onchange(Some(picker._change.as_ref().unchecked_ref()));
    picker.input.add_event_listener_with_callback("cancel", picker._cancel.as_ref().unchecked_ref()).map_err(|e| format!("{e:?}"))?;
    Ok(picker)
}

async fn read(file: &web_sys::File, inbox: &Inbox, pending: &AtomicBool) -> Result<Vec<u8>, String> {
    c0_host::validate_name(&file.name())?;
    let size = file.size();
    if !size.is_finite() || size < 0.0 || size.fract() != 0.0 || size > c0_host::MAX_FILE_BYTES.saturating_sub(held(inbox)) as f64 {
        return Err("picked file exceeds the host file limit".into());
    }
    let budget = c0_host::reserve_file_bytes(size as usize, held(inbox))?;
    let buffer = JsFuture::from(file.array_buffer()).await.map_err(|e| format!("could not read picked file: {e:?}"))?;
    if !pending.load(Ordering::SeqCst) {
        return Err("file chooser was retired".into());
    }
    let buffer = js_sys::Uint8Array::new(&buffer);
    if buffer.length() as usize > budget.bytes() || buffer.length() as usize > c0_host::MAX_FILE_BYTES.saturating_sub(held(inbox)) {
        return Err("file exceeds the host inbox limit".into());
    }
    Ok(buffer.to_vec())
}
