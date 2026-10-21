//! An event listener borrows one unpainted egui pass, even when animation frames are paused.

use deckcraft_ui_egui::{ControlRequest, SlideApp};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc::{Sender, TryRecvError, channel},
};
use wasm_bindgen::{JsCast as _, prelude::*};

const PAGE_FUNCTION: &str = "__deckcraftControl";
const PUMP_EVENT: &str = "deckcraft-control-pass";
const MAX_PENDING: usize = 32;
const TIMEOUT_MS: f64 = 10_000.0;
const POLL_MS: i32 = 4;
const INPUT_PASSES: usize = 40;

thread_local! {
    static RUNNER: RefCell<Option<eframe::WebRunner>> = const { RefCell::new(None) };
    static PUMP: RefCell<Option<Pump>> = const { RefCell::new(None) };
}

pub fn runner(runner: &eframe::WebRunner) {
    RUNNER.with(|slot| *slot.borrow_mut() = Some(runner.clone()));
}

#[derive(Clone)]
struct Pump {
    target: web_sys::EventTarget,
    ran: Rc<Cell<bool>>,
}

impl Pump {
    fn get() -> Option<Self> {
        if let Some(pump) = PUMP.with(|slot| slot.borrow().clone()) {
            return Some(pump);
        }
        let runner = RUNNER.with(|slot| slot.borrow().clone())?;
        let pump = Self { target: web_sys::EventTarget::new().ok()?, ran: Rc::default() };
        let ran = pump.ran.clone();
        runner
            .add_event_listener::<web_sys::Event>(&pump.target, PUMP_EVENT, move |_, app_runner| {
                app_runner.logic();
                ran.set(true);
            })
            .ok()?;
        PUMP.with(|slot| *slot.borrow_mut() = Some(pump.clone()));
        Some(pump)
    }
    fn pass(&self) -> bool {
        self.ran.set(false);
        if let Ok(event) = web_sys::Event::new(PUMP_EVENT) {
            let _ = self.target.dispatch_event(&event);
        }
        self.ran.get()
    }
}

pub fn attach(mut app: SlideApp, ctx: &egui::Context, filters: [&'static [&'static str]; 4]) -> SlideApp {
    let (tx, rx) = channel();
    super::files::attach(&mut app, ctx, filters);
    install(tx, ctx.clone());
    app.with_control(rx)
}

fn install(tx: Sender<ControlRequest>, ctx: egui::Context) {
    let Some(window) = web_sys::window() else { return };
    let pending = Rc::new(Cell::new(0usize));
    let call = Closure::<dyn FnMut(JsValue) -> js_sys::Promise>::new(move |text: JsValue| {
        let parsed = text.as_string().ok_or_else(|| "request must be JSON text".to_owned()).and_then(|s| super::parse(&s));
        let (method, params) = match parsed {
            Ok(v) => v,
            Err(e) => return resolved(error(e)),
        };
        if pending.get() >= MAX_PENDING {
            return resolved(error("too many pending page requests"));
        }
        let input = matches!(method.as_str(), "ui.click" | "ui.move" | "ui.drag" | "ui.key" | "ui.text");
        let (request, rx) = ControlRequest::new(method, params);
        let state = request.state.clone();
        if tx.send(request).is_err() {
            return resolved(error("DeckCraft has stopped"));
        }
        let pump = Pump::get();
        if pump.as_ref().is_some_and(Pump::pass) {
            if input {
                for _ in 0..INPUT_PASSES {
                    if !pump.as_ref().is_some_and(Pump::pass) {
                        break;
                    }
                }
            }
        } else {
            ctx.request_repaint();
        }
        super::files::retire();
        match rx.try_recv() {
            Ok(reply) => resolved(reply),
            Err(TryRecvError::Disconnected) => resolved(error("DeckCraft dropped the request")),
            Err(TryRecvError::Empty) => {
                pending.set(pending.get().saturating_add(1));
                let pending = pending.clone();
                wasm_bindgen_futures::future_to_promise(async move {
                    let start = js_sys::Date::now();
                    let reply = loop {
                        match rx.try_recv() {
                            Ok(reply) => break reply,
                            Err(TryRecvError::Disconnected) => break error("DeckCraft dropped the request"),
                            Err(TryRecvError::Empty) => {}
                        }
                        if js_sys::Date::now() - start >= TIMEOUT_MS {
                            ControlRequest::abandon(&state);
                            break error("page request timed out");
                        }
                        sleep().await;
                        if let Some(pump) = &pump {
                            pump.pass();
                        }
                    };
                    pending.set(pending.get().saturating_sub(1));
                    Ok(JsValue::from_str(&reply.to_string()))
                })
            }
        }
    });
    if let Err(e) = js_sys::Reflect::set(&window, &JsValue::from_str(PAGE_FUNCTION), call.as_ref().unchecked_ref()) {
        log::error!("page-control: install failed: {e:?}");
    } else {
        call.forget();
        if let Ok(ready) = web_sys::Event::new("deckcraft-control-ready") {
            let _ = window.dispatch_event(&ready);
        }
    }
}

fn error(message: impl std::fmt::Display) -> Value {
    json!({"ok": false, "error": message.to_string()})
}
fn resolved(value: Value) -> js_sys::Promise {
    js_sys::Promise::resolve(&JsValue::from_str(&value.to_string()))
}

async fn sleep() {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window()
            && window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, POLL_MS).is_ok()
        {
            return;
        }
        let _ = resolve.call0(&JsValue::NULL);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}
