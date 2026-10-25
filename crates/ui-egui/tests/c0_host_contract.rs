use deckcraft_engine::Session;
use deckcraft_ui_egui::{
    ControlRequest, Services, SlideApp, c0_host,
    control::{self, Outcome},
};
use serde_json::{Value, json};

fn app() -> SlideApp {
    let mut app = SlideApp::new(Session::new(), Services::default());
    app.run("file.new", json!({})).unwrap();
    app
}

fn reply(app: &mut SlideApp, ctx: &egui::Context, method: &str, params: Value) -> Value {
    let (request, _) = ControlRequest::new(method, params);
    let Outcome::Done(value) = control::handle(app, ctx, &request) else { panic!("unexpected deferred reply") };
    value
}

fn call(app: &mut SlideApp, ctx: &egui::Context, method: &str, params: Value) -> Value {
    let value = reply(app, ctx, method, params);
    assert_eq!(value["ok"], true, "{value}");
    value["result"].clone()
}

#[test]
fn inspect_tracks_slides_sections_selection_history_and_revision() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.run("slide.new", json!({"title":"Second"})).unwrap();
    app.run("slide.hide", json!({"index":1, "hidden":true})).unwrap();
    app.run("section.add", json!({"at":0,"name":"Opening"})).unwrap();
    let result = call(&mut app, &ctx, "c0.inspect", json!({}));
    let state = app.session.active().unwrap();
    assert_eq!(result["revision"], state.revision);
    assert_eq!(result["slideCount"], 2);
    assert_eq!(result["currentSlide"], 1);
    assert_eq!(result["slides"][1]["title"], "Second");
    assert_eq!(result["slides"][1]["hidden"], true);
    assert_eq!(result["slides"][0]["section"], "Opening");
    assert_eq!(result["sections"][0], json!({"name":"Opening","first":0,"count":2}));
    assert_eq!(result["dirty"], true);
    assert_eq!(result["canUndo"], true);
    assert_eq!(result["canRedo"], false);
    assert!(result["panes"]["notes"].is_boolean());
    assert_eq!(result["panes"]["thumbnails"], true);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(call(&mut app, &ctx, "c0.inspect", json!({}))["canRedo"], true);
}

#[test]
fn empty_session_inspection_and_missing_document_errors() {
    let mut app = SlideApp::new(Session::new(), Services::default());
    let ctx = egui::Context::default();
    let inspected = call(&mut app, &ctx, "c0.inspect", json!({}));
    assert_eq!(inspected["slides"], json!([]));
    assert_eq!(inspected["currentSlide"], Value::Null);
    assert_eq!(inspected["revision"], 0);
    assert_eq!(inspected["dirty"], false);
    for (method, params) in [("c0.documentBytes", json!({})), ("c0.goTo", json!({"slide":0})), ("c0.show", json!({"action":"start"}))] {
        assert_eq!(reply(&mut app, &ctx, method, params)["ok"], false);
    }
}

#[test]
fn go_to_accepts_indices_and_stable_ids_and_rejects_hostile_values() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.run("slide.new", json!({"title":"Second"})).unwrap();
    let id = app.session.active().unwrap().doc.slides[0].id.0;
    for slide in [json!(0), json!(id), json!(id.to_string()), json!({"id":id})] {
        assert_eq!(call(&mut app, &ctx, "c0.goTo", json!({"slide":slide}))["index"], 0);
    }
    for slide in [json!(-1), json!(1.5), json!(u64::MAX), json!("bad"), json!(false), json!({"id":u64::MAX})] {
        assert_eq!(reply(&mut app, &ctx, "c0.goTo", json!({"slide":slide}))["ok"], false);
    }
    assert_eq!(app.session.active().unwrap().selection.slide, 0);
}

#[test]
fn snapshots_are_read_only_and_round_trip_both_formats() {
    for format in ["pptx", "deckcraft"] {
        let mut app = app();
        let ctx = egui::Context::default();
        app.run("slide.new", json!({"title":"Round trip"})).unwrap();
        let before = c0_host::inspect(&app);
        let snapshot = call(&mut app, &ctx, "c0.documentBytes", json!({"format":format}));
        assert_eq!(c0_host::inspect(&app), before);
        assert_eq!(snapshot["format"], format);
        assert_eq!(snapshot["revision"], before["revision"]);
        let mut target = SlideApp::new(Session::new(), Services::default());
        call(&mut target, &ctx, "c0.openFile", json!({"name":snapshot["name"],"bytes":snapshot["bytes"]}));
        assert_eq!(target.session.documents().len(), 1);
        assert_eq!(target.session.active().unwrap().doc.slides.len(), 2);
        assert_eq!(target.session.active().unwrap().doc.slides[1].title(), "Round trip");
        assert!(!target.session.active().unwrap().is_dirty());
    }
}

#[test]
fn default_snapshot_is_pptx_and_invalid_formats_are_errors() {
    let mut app = app();
    let ctx = egui::Context::default();
    assert_eq!(call(&mut app, &ctx, "c0.documentBytes", json!({}))["format"], "pptx");
    for format in [json!("pdf"), Value::Null, json!(42)] {
        assert_eq!(reply(&mut app, &ctx, "c0.documentBytes", json!({"format":format}))["ok"], false);
    }
}

#[test]
fn snapshot_filenames_are_bounded_basename_utf8_and_can_be_reopened() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.session.doc_mut().unwrap().title = format!("a/b\\c{}", "é".repeat(200));
    let snapshot = call(&mut app, &ctx, "c0.documentBytes", json!({"format":"deckcraft"}));
    let name = snapshot["name"].as_str().unwrap();
    assert!(name.len() <= c0_host::MAX_NAME_BYTES);
    assert!(!name.contains(['/', '\\', '\0']));
    call(&mut app, &ctx, "c0.openFile", json!({"name":name,"bytes":snapshot["bytes"]}));
}

#[test]
fn open_includes_outline_importers_and_replaces_only_the_active_document() {
    for name in ["outline.txt", "outline.md"] {
        let mut app = app();
        let ctx = egui::Context::default();
        app.run("file.new", json!({})).unwrap();
        let prior = app.session.documents()[0].uid;
        let bytes = deckcraft_engine::cmd::base64_encode(b"# Opening\n\nHello\n\n# Closing\n\nBye\n");
        call(&mut app, &ctx, "c0.openFile", json!({"name":name,"bytes":bytes}));
        assert_eq!(app.session.documents().len(), 2);
        assert_eq!(app.session.documents()[0].uid, prior);
        assert_eq!(app.session.active().unwrap().title(), "outline");
    }
}

#[test]
fn invalid_imports_never_prompt_or_replace_dirty_work() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.run("slide.new", json!({"title":"Keep me"})).unwrap();
    let before = c0_host::inspect(&app);
    for name in ["../bad.pptx", "a/b.pptx", "a\\b.pptx", "", ".", "..", "x\0.pptx", "bad.pptx"] {
        assert_eq!(reply(&mut app, &ctx, "c0.openFile", json!({"name":name,"bytes":"aGVsbG8="}))["ok"], false);
        assert_eq!(c0_host::inspect(&app), before);
        assert!(app.dialog.is_none());
    }
    assert_eq!(reply(&mut app, &ctx, "c0.openFile", json!({"name":"a.pptx","bytes":"*invalid*"}))["ok"], false);
}

#[test]
fn malformed_base64_cannot_turn_into_an_empty_outline_document() {
    let mut app = app();
    let ctx = egui::Context::default();
    let before = c0_host::inspect(&app);
    for bytes in ["x", "A===", "AA=A", "====", "Zh==", "AAAA ", "data:text/plain;base64,aGk=", "-_=="] {
        assert_eq!(reply(&mut app, &ctx, "c0.openFile", json!({"name":"bad.txt","bytes":bytes}))["ok"], false, "{bytes}");
        assert_eq!(c0_host::inspect(&app), before);
        assert!(app.dialog.is_none());
    }
}

#[test]
fn new_document_is_blank_and_rejects_unknown_templates() {
    let mut app = app();
    let ctx = egui::Context::default();
    for params in [json!({}), json!({"template":"blank"}), json!({"template":""})] {
        call(&mut app, &ctx, "c0.newDocument", params);
        assert_eq!(app.session.documents().len(), 1);
        assert_eq!(app.session.active().unwrap().doc.slides.len(), 1);
        assert_eq!(app.session.active().unwrap().doc.slides[0].title(), "");
        assert!(app.backstage.is_none());
    }
    assert_eq!(reply(&mut app, &ctx, "c0.newDocument", json!({"template":"unknown"}))["ok"], false);
}

#[test]
fn switching_documents_retires_the_replacement_and_its_old_close_prompt() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.run("file.new", json!({})).unwrap();
    app.run("slide.new", json!({"title":"Keep this document"})).unwrap();
    let uid = app.session.active().unwrap().uid;
    assert_eq!(call(&mut app, &ctx, "c0.newDocument", json!({}))["pending"], "saveChanges");
    app.run("window.next", json!({"index":0})).unwrap();
    ctx.run_ui(Default::default(), |ui| app.logic(ui.ctx())).textures_delta.clear();
    assert!(app.dialog.is_none());
    call(&mut app, &ctx, "c0.newDocument", json!({}));
    assert_eq!(app.session.documents().len(), 2);
    let retained = app.session.documents().iter().find(|d| d.uid == uid).unwrap();
    assert!(retained.is_dirty());
    assert_eq!(retained.doc.slides.len(), 2);
}

#[test]
fn cancellation_retires_picker_and_app_ticket_but_keeps_unsaved_prompt() {
    let mut app = app();
    let ctx = egui::Context::default();
    assert_eq!(call(&mut app, &ctx, "c0.cancelFileDialog", json!({})), json!({"cancelled":false}));
    let old = c0_host::begin_picker(&ctx);
    let live = c0_host::begin_picker(&ctx);
    assert!(!old.load(std::sync::atomic::Ordering::SeqCst));
    let id = app.begin_external_file("open");
    app.services.inbox = Some(Default::default());
    assert_eq!(call(&mut app, &ctx, "c0.cancelFileDialog", json!({})), json!({"cancelled":true}));
    assert!(!live.load(std::sync::atomic::Ordering::SeqCst));
    let before = c0_host::inspect(&app);
    app.services.inbox.as_ref().unwrap().lock().unwrap().push(deckcraft_ui_egui::InboxEvent::File(deckcraft_ui_egui::InboxFile {
        id,
        name: "late.md".into(),
        bytes: b"# Late".to_vec(),
    }));
    for _ in 0..3 {
        ctx.run_ui(Default::default(), |ui| app.logic(ui.ctx())).textures_delta.clear();
    }
    assert_eq!(c0_host::inspect(&app), before);
    app.run("slide.new", json!({})).unwrap();
    assert_eq!(call(&mut app, &ctx, "c0.newDocument", json!({}))["pending"], "saveChanges");
    call(&mut app, &ctx, "c0.cancelFileDialog", json!({}));
    assert_eq!(app.dialog.as_ref().unwrap().id, "close");
}

#[test]
fn slide_show_start_end_and_bad_indices() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.run("slide.new", json!({})).unwrap();
    assert_eq!(call(&mut app, &ctx, "c0.show", json!({"action":"start","from":1})), json!({"showing":true}));
    assert!(app.show.is_some());
    assert_eq!(call(&mut app, &ctx, "c0.inspect", json!({}))["showing"], true);
    assert_eq!(call(&mut app, &ctx, "c0.show", json!({"action":"end"})), json!({"showing":false}));
    for params in [json!({"action":"oops"}), json!({"action":"start","from":999}), json!({"action":"start","from":-1})] {
        assert_eq!(reply(&mut app, &ctx, "c0.show", params)["ok"], false);
    }
}

#[test]
fn unknown_methods_and_wrong_params_use_normal_error_envelopes() {
    let mut app = app();
    let ctx = egui::Context::default();
    for (method, params) in [("c0.unknown", json!({})), ("c0.inspect", json!([]))] {
        let result = reply(&mut app, &ctx, method, params);
        assert_eq!(result["ok"], false);
        assert!(result["error"].is_string());
    }
}

// ---------- c0.selection / c0.place (AI actions) ----------

const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
const JPEG_1X1: &str = "/9j/4AAQSkZJRgABAgAAAQABAAD//gAPTGF2YzYxLjMuMTAwAP/bAEMACAQEBAQEBQUFBQUFBgYGBgYGBgYGBgYGBgcHBwgICAcHBwYGBwcICAgICQkJCAgICAkJCgoKDAwLCw4ODhERFP/EAEwAAQEAAAAAAAAAAAAAAAAAAAAGAQEBAAAAAAAAAAAAAAAAAAAGBxABAAAAAAAAAAAAAAAAAAAAABEBAAAAAAAAAAAAAAAAAAAAAP/AABEIAAIABAMBIgACEQADEQD/2gAMAwEAAhEDEQA/AIsAUX9//9k=";

fn undo_len(app: &SlideApp) -> usize {
    app.session.active().unwrap().history.undo.len()
}

fn deck() -> (SlideApp, egui::Context) {
    let mut app = app();
    app.run("slide.new", json!({"layout":"titleAndContent","title":"Second","body":"one\ntwo"})).unwrap();
    app.run("slide.notes", json!({"text":"say hello"})).unwrap();
    (app, egui::Context::default())
}

#[test]
fn ai_methods_are_registered() {
    assert_eq!(c0_host::ai_methods(), ["c0.selection", "c0.place"]);
    assert_eq!(c0_host::MAX_PICTURE_BYTES, 40 * 1024 * 1024);
}

#[test]
fn selection_reads_slide_outline_notes_and_selected_text() {
    let (mut app, ctx) = deck();
    let s = call(&mut app, &ctx, "c0.selection", json!({}));
    assert_eq!(s["kind"], "slide");
    assert_eq!(s["slideIndex"], 1);
    assert_eq!(s["slideCount"], 2);
    assert_eq!(s["title"], "Second");
    assert_eq!(s["bodyText"], "one\ntwo");
    assert_eq!(s["notes"], "say hello");
    assert_eq!(s["outline"][1], json!({"index":1,"title":"Second"}));
    assert!(s.get("selectedText").is_none());
    // A selected text box gives its whole text; a selected range gives just the range.
    let id = app.session.active().unwrap().doc.slides[1].shapes.iter().find(|x| x.ph_type().is_some_and(|k| k.is_title())).unwrap().id;
    app.run("text.edit", json!({"id": id})).unwrap();
    app.run("text.selectAll", json!({})).unwrap();
    assert_eq!(call(&mut app, &ctx, "c0.selection", json!({}))["selectedText"], "Second");
    app.run("text.edit", json!({"id": id, "at": [0, 1]})).unwrap();
    app.run("text.move", json!({"to": "right", "extend": true})).unwrap();
    assert_eq!(call(&mut app, &ctx, "c0.selection", json!({}))["selectedText"], "e");
    let mut empty = SlideApp::new(Session::new(), Services::default());
    assert_eq!(reply(&mut empty, &ctx, "c0.selection", json!({}))["ok"], false);
}

#[test]
fn place_slides_is_one_undo_step_after_the_current_slide() {
    let (mut app, ctx) = deck();
    app.run("slide.go", json!({"index": 0})).unwrap();
    let (before, steps) = (app.session.active().unwrap().doc.slides.len(), undo_len(&app));
    let r = call(&mut app, &ctx, "c0.place", json!({"slides":[{"title":"A","bullets":["a1","a2"],"notes":"na"},{"title":"B","bullets":[]}]}));
    assert_eq!(r["placed"], "slides");
    assert_eq!(r["indices"], json!([1, 2]));
    let st = app.session.active().unwrap();
    assert_eq!(st.doc.slides.len(), before + 2);
    assert_eq!(st.doc.slides[1].title(), "A");
    assert_eq!(st.doc.slides[2].title(), "B");
    assert_eq!(st.doc.slides[3].title(), "Second");
    assert_eq!(st.doc.slides[1].notes_text(), "na");
    assert!(st.is_dirty());
    assert_eq!(undo_len(&app), steps + 1);
    app.run("edit.undo", json!({})).unwrap();
    let st = app.session.active().unwrap();
    assert_eq!(st.doc.slides.len(), before);
    assert_eq!(st.doc.slides[1].title(), "Second");
    // `after` picks the place.
    call(&mut app, &ctx, "c0.place", json!({"slides":[{"title":"Last","bullets":["x"]}],"after":1}));
    assert_eq!(app.session.active().unwrap().doc.slides[2].title(), "Last");
}

#[test]
fn place_replaces_selected_text_and_sets_notes_each_as_one_step() {
    let (mut app, ctx) = deck();
    let id = app.session.active().unwrap().doc.slides[1].shapes.iter().find(|x| x.ph_type().is_some_and(|k| k.is_title())).unwrap().id;
    app.run("text.edit", json!({"id": id})).unwrap();
    app.run("text.selectAll", json!({})).unwrap();
    let steps = undo_len(&app);
    call(&mut app, &ctx, "c0.place", json!({"text":"Better title","mode":"replace-selection"}));
    assert_eq!(app.session.active().unwrap().doc.slides[1].title(), "Better title");
    assert_eq!(undo_len(&app), steps + 1);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(app.session.active().unwrap().doc.slides[1].title(), "Second");
    // A selected text box (no range) is replaced whole.
    app.run("text.exit", json!({})).unwrap();
    app.run("shape.select", json!({"ids":[id]})).ok();
    app.session.select(|_, sel| {
        sel.text = None;
        sel.shapes = vec![id];
    })
    .unwrap();
    call(&mut app, &ctx, "c0.place", json!({"text":"Whole","mode":"replace-selection"}));
    assert_eq!(app.session.active().unwrap().doc.slides[1].title(), "Whole");
    // Notes.
    let steps = undo_len(&app);
    let r = call(&mut app, &ctx, "c0.place", json!({"notes":"new notes","slideIndex":0}));
    assert_eq!(r["slideIndex"], 0);
    assert_eq!(app.session.active().unwrap().doc.slides[0].notes_text(), "new notes");
    assert_eq!(undo_len(&app), steps + 1);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(app.session.active().unwrap().doc.slides[0].notes_text(), "");
}

#[test]
fn place_picture_is_centred_fitted_and_one_undo_step() {
    let (mut app, ctx) = deck();
    let (shapes, steps) = (app.session.active().unwrap().current_slide().unwrap().shapes.len(), undo_len(&app));
    let r = call(&mut app, &ctx, "c0.place", json!({"png": PNG_1X1, "name": "AI image"}));
    assert_eq!(r["placed"], "picture");
    let st = app.session.active().unwrap();
    assert_eq!(st.current_slide().unwrap().shapes.len(), shapes + 1);
    let size = st.doc.slide_size;
    let (x, y, w, h) = (r["rect"]["x"].as_f64().unwrap(), r["rect"]["y"].as_f64().unwrap(), r["rect"]["width"].as_f64().unwrap(), r["rect"]["height"].as_f64().unwrap());
    assert!(w <= size.width + 0.01 && h <= size.height + 0.01 && x >= -0.01 && y >= -0.01);
    assert!((x + w / 2.0 - size.width / 2.0).abs() < 0.01 && (y + h / 2.0 - size.height / 2.0).abs() < 0.01);
    assert!(st.is_dirty());
    assert_eq!(undo_len(&app), steps + 1);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(app.session.active().unwrap().current_slide().unwrap().shapes.len(), shapes);
    let r = call(&mut app, &ctx, "c0.place", json!({"jpeg": JPEG_1X1, "name": "photo.jpg"}));
    assert_eq!(r["placed"], "picture");
}

#[test]
fn place_rejects_bad_params_with_plain_messages_and_changes_nothing() {
    let (mut app, ctx) = deck();
    let (revision_steps, slides) = (undo_len(&app), app.session.active().unwrap().doc.slides.len());
    let huge = "A".repeat(60 * 1024 * 1024);
    for params in [
        json!({}),
        json!({"slides": []}),
        json!({"slides": "x"}),
        json!({"slides": [{"bullets": []}]}),
        json!({"slides": [{"title": "t", "bullets": [1]}]}),
        json!({"slides": [{"title": "t"}], "after": 99}),
        json!({"slides": [{"title": "t"}], "text": "x", "mode": "replace-selection"}),
        json!({"text": "x"}),
        json!({"text": "x", "mode": "replace-selection"}),
        json!({"text": "", "mode": "replace-selection"}),
        json!({"notes": 4}),
        json!({"notes": "n", "slideIndex": 9}),
        json!({"png": "!!!", "name": "a"}),
        json!({"png": PNG_1X1}),
        json!({"png": PNG_1X1, "name": "../x"}),
        json!({"jpeg": PNG_1X1, "name": "a"}),
        json!({"png": huge, "name": "a"}),
    ] {
        let r = reply(&mut app, &ctx, "c0.place", params.clone());
        assert_eq!(r["ok"], false, "{params:.80}");
        assert!(r["error"].as_str().is_some_and(|e| !e.is_empty()), "{r:.200}");
    }
    assert_eq!(undo_len(&app), revision_steps);
    assert_eq!(app.session.active().unwrap().doc.slides.len(), slides);
    let mut empty = SlideApp::new(Session::new(), Services::default());
    assert_eq!(reply(&mut empty, &ctx, "c0.place", json!({"notes":"x"}))["ok"], false);
}

#[test]
fn a_refused_placement_changes_nothing() {
    let (mut app, ctx) = deck();
    let (steps, before) = (undo_len(&app), app.session.active().unwrap().doc.clone());
    // A bad second slide is refused before the first is added.
    let r = reply(&mut app, &ctx, "c0.place", json!({"slides":[{"title":"ok"},{"title":"bad","notes":5}]}));
    assert_eq!(r["ok"], false);
    assert_eq!(undo_len(&app), steps);
    assert!(std::sync::Arc::ptr_eq(&app.session.active().unwrap().doc, &before));
}
