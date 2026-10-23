//! Tool modes (`toolbar`, `toolbar_double`, `toolbar_floating`) and the foldable right dock
//! (`right_dock_collapsed`) through `c0.set` / `c0.inspect`, and what they draw.
use deckcraft_engine::Session;
use deckcraft_ui_egui::{
    ControlRequest, Services, SlideApp,
    control::{self, Outcome},
};
use egui::{Event, PointerButton, Pos2, Shape, vec2};
use egui_kittest::kittest::Queryable as _;
use serde_json::{Value, json};

type Harness = egui_kittest::Harness<'static, SlideApp>;

fn app() -> SlideApp {
    let mut app = SlideApp::new(Session::new(), Services::default());
    app.run("file.new", json!({})).unwrap();
    app
}

fn harness() -> Harness {
    let mut h = egui_kittest::Harness::builder().with_size(vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut SlideApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app(),
    );
    h.run_steps(4);
    h
}

fn reply(h: &mut Harness, method: &str, params: Value) -> Value {
    let ctx = h.ctx.clone();
    let (request, _) = ControlRequest::new(method, params);
    let Outcome::Done(value) = control::handle(h.state_mut(), &ctx, &request) else { panic!("unexpected deferred reply") };
    value
}

fn set(h: &mut Harness, panels: Value) -> Value {
    let value = reply(h, "c0.set", json!({"panels": panels}));
    assert_eq!(value["ok"], true, "{value}");
    h.run_steps(4);
    value["result"].clone()
}

fn inspect(h: &mut Harness) -> Value {
    reply(h, "c0.inspect", json!({}))["result"].clone()
}

/// Where a drawn text starts, if it is drawn.
fn text_pos(h: &Harness, wanted: &str) -> Option<Pos2> {
    fn find(shape: &Shape, wanted: &str) -> Option<Pos2> {
        match shape {
            Shape::Text(t) if t.galley.text() == wanted => Some(t.pos),
            Shape::Vec(v) => v.iter().find_map(|s| find(s, wanted)),
            _ => None,
        }
    }
    h.output().shapes.iter().find_map(|s| find(&s.shape, wanted))
}

fn click(h: &mut Harness, at: Pos2) {
    h.event(Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() });
        h.run_steps(2);
    }
}

#[test]
fn defaults_are_the_upstream_layout() {
    let mut h = harness();
    let panes = inspect(&mut h)["panes"].clone();
    assert_eq!(panes["toolbar"], true);
    assert_eq!(panes["toolbar_double"], false);
    assert_eq!(panes["toolbar_floating"], false);
    assert_eq!(panes["right_dock_collapsed"], false);
    assert_eq!(inspect(&mut h)["toolbarFloat"]["pos"], Value::Null);
    // The full card is drawn in the dock, and no dock strip exists.
    assert!(text_pos(&h, "Paste").is_some());
    assert!(h.query_by_label("Unfold panels").is_none());
}

#[test]
fn every_new_key_sets_reads_back_and_resets() {
    let mut h = harness();
    for key in ["toolbar", "toolbar_double", "toolbar_floating", "right_dock_collapsed"] {
        let off = key == "toolbar";
        let reply = set(&mut h, json!({key: !off}));
        assert_eq!(reply["applied"], json!([format!("panels.{key}")]), "{key}");
        assert_eq!(inspect(&mut h)["panes"][key], !off, "{key}");
        set(&mut h, json!({key: off}));
        assert_eq!(inspect(&mut h)["panes"][key], off, "{key}");
    }
}

#[test]
fn unknown_and_mistyped_keys_are_reported_not_applied() {
    let mut h = harness();
    let reply = set(&mut h, json!({"toolbar_triple": true, "toolbar_double": "yes", "right_dock_collapsed": 1, "toolbar": true}));
    assert_eq!(reply["unknown"], json!(["panels.toolbar_triple"]));
    let mut invalid: Vec<_> = reply["invalid"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    invalid.sort_unstable();
    assert_eq!(invalid, ["panels.right_dock_collapsed", "panels.toolbar_double"]);
    assert_eq!(reply["applied"], json!(["panels.toolbar"]));
    let panes = inspect(&mut h)["panes"].clone();
    assert_eq!(panes["toolbar_double"], false);
    assert_eq!(panes["right_dock_collapsed"], false);
}

#[test]
fn the_modes_are_not_saved_in_the_document_or_the_ribbon_state() {
    let mut h = harness();
    let before = h.state().ui.ribbon_collapsed;
    set(&mut h, json!({"toolbar": false, "toolbar_double": true, "toolbar_floating": true, "right_dock_collapsed": true}));
    assert_eq!(h.state().ui.ribbon_collapsed, before);
    let bytes = reply(&mut h, "c0.documentBytes", json!({"format": "deckcraft"}));
    assert_eq!(bytes["ok"], true);
    assert!(!bytes.to_string().contains("toolbar"));
}

#[test]
fn toolbar_off_folds_the_card_to_tabs_and_tab_clicks_do_not_bring_it_back() {
    let mut h = harness();
    set(&mut h, json!({"toolbar": false}));
    assert!(text_pos(&h, "Paste").is_none());
    let insert = text_pos(&h, "Insert").expect("tabs stay");
    click(&mut h, insert + vec2(8.0, 4.0));
    assert!(text_pos(&h, "Paste").is_none());
    set(&mut h, json!({"toolbar": true}));
    assert!(text_pos(&h, "Paste").is_some() || text_pos(&h, "Table").is_some());
}

#[test]
fn compact_draws_the_same_ribbon_smaller_in_the_dock() {
    let mut h = harness();
    let tab_gap = |h: &Harness| text_pos(h, "Insert").unwrap().x - text_pos(h, "Home").unwrap().x;
    let full = tab_gap(&h);
    set(&mut h, json!({"toolbar_double": true}));
    let compact = tab_gap(&h);
    assert!((compact / full - 0.8).abs() < 0.02, "full {full}, compact {compact}");
    // Still a working ribbon: a tab click switches the card.
    let insert = text_pos(&h, "Insert").unwrap();
    click(&mut h, insert + vec2(8.0, 3.0));
    assert_eq!(h.state().ui.tab, "Insert");
    assert!(text_pos(&h, "Table").is_some());
}

#[test]
fn floating_lifts_the_ribbon_off_the_dock_and_the_grip_moves_it() {
    let mut h = harness();
    let docked = text_pos(&h, "Home").unwrap();
    set(&mut h, json!({"toolbar_floating": true}));
    let floating = text_pos(&h, "Home").expect("ribbon is still drawn");
    assert!(floating.y > docked.y, "the strip sits over the canvas area, below the title bar");
    assert!(text_pos(&h, "Paste").is_some());
    // Drag the grip 100 points right and 40 down.
    let start = text_pos(&h, "· · ·").expect("the grip is drawn") + vec2(10.0, 4.0);
    h.event(Event::PointerMoved(start));
    h.run_steps(2);
    h.event(Event::PointerButton { pos: start, button: PointerButton::Primary, pressed: true, modifiers: Default::default() });
    h.run_steps(2);
    h.event(Event::PointerMoved(start + vec2(100.0, 40.0)));
    h.run_steps(3);
    h.event(Event::PointerButton { pos: start + vec2(100.0, 40.0), button: PointerButton::Primary, pressed: false, modifiers: Default::default() });
    h.run_steps(3);
    let moved = text_pos(&h, "Home").unwrap();
    assert!((moved.x - floating.x - 100.0).abs() < 2.0 && (moved.y - floating.y - 40.0).abs() < 2.0, "{floating:?} -> {moved:?}");
    assert!(inspect(&mut h)["toolbarFloat"]["pos"].is_array());
    // Back to docked: the ribbon returns to where it was.
    set(&mut h, json!({"toolbar_floating": false}));
    assert_eq!(text_pos(&h, "Home"), Some(docked));
}

#[test]
fn floating_and_compact_combine() {
    let mut h = harness();
    set(&mut h, json!({"toolbar_floating": true}));
    let wide = text_pos(&h, "Insert").unwrap().x - text_pos(&h, "Home").unwrap().x;
    set(&mut h, json!({"toolbar_double": true}));
    let narrow = text_pos(&h, "Insert").unwrap().x - text_pos(&h, "Home").unwrap().x;
    assert!((narrow / wide - 0.8).abs() < 0.02, "{wide} {narrow}");
}

#[test]
fn the_dock_folds_to_an_icon_strip_and_unfolds_with_its_pane() {
    let mut h = harness();
    h.state_mut().run("view.pane", json!({"pane": "animation"})).unwrap();
    h.run_steps(3);
    assert!(text_pos(&h, "Animation Pane").is_some());
    set(&mut h, json!({"right_dock_collapsed": true}));
    assert!(text_pos(&h, "Animation Pane").is_none(), "the pane is folded away");
    assert_eq!(inspect(&mut h)["panes"]["right_dock_collapsed"], true);
    for label in ["Unfold panels", "Format Shape", "Selection", "Comments"] {
        assert!(h.query_all_by_label(label).next().is_some(), "{label}");
    }
    // Unfolding with the host switch brings the same pane back.
    set(&mut h, json!({"right_dock_collapsed": false}));
    assert!(text_pos(&h, "Animation Pane").is_some());
    assert!(h.query_by_label("Unfold panels").is_none());
}

#[test]
fn a_strip_icon_opens_its_pane_and_unfolds() {
    let mut h = harness();
    set(&mut h, json!({"right_dock_collapsed": true}));
    let icon = h.get_by_label("Selection").rect().center();
    click(&mut h, icon);
    assert_eq!(h.state().ui.pane.as_deref(), Some("selection"));
    assert_eq!(inspect(&mut h)["panes"]["right_dock_collapsed"], false);
    assert!(h.query_by_label("Unfold panels").is_none());
}

#[test]
fn folding_does_not_move_the_ribbon_or_resize_the_canvas_by_more_than_the_strip() {
    let mut h = harness();
    let ribbon = text_pos(&h, "Home").unwrap();
    set(&mut h, json!({"right_dock_collapsed": true}));
    assert_eq!(text_pos(&h, "Home"), Some(ribbon));
    set(&mut h, json!({"right_dock_collapsed": false}));
    assert_eq!(text_pos(&h, "Home"), Some(ribbon));
}
