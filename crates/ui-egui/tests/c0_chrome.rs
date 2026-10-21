use deckcraft_engine::Session;
use deckcraft_ui_egui::{
    Services, SlideApp,
    c0_host::{self, Chrome},
    control,
};
use egui_kittest::kittest::Queryable as _;
use serde_json::json;

fn app() -> SlideApp {
    let mut app = SlideApp::new(Session::new(), Services::default());
    app.run("file.new", json!({})).unwrap();
    app
}

fn harness(app: SlideApp) -> egui_kittest::Harness<'static, SlideApp> {
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut SlideApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

fn text(h: &egui_kittest::Harness<'_, SlideApp>) -> String {
    fn collect(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push('\n');
            }
            egui::Shape::Vec(v) => {
                for s in v {
                    collect(s, out);
                }
            }
            _ => {}
        }
    }
    let mut out = String::new();
    for s in &h.output().shapes {
        collect(&s.shape, &mut out);
    }
    out
}

#[test]
fn defaults_keep_upstream_chrome_and_about_links() {
    let app = app();
    assert_eq!(app.chrome, Chrome { title_bar: true, file_tab: true, start_screen: true, community_links: true, status_bar: true });
    let mut h = harness(app);
    assert!(h.query_by_label("File").is_some());
    h.state_mut().run("app.about", json!({})).unwrap();
    h.run_steps(3);
    assert!(h.query_by_label("Source on GitHub").is_some());
    assert!(h.query_by_label("Join our community on Discord").is_some());
}

#[test]
fn hidden_community_removes_about_backstage_and_learn_more_links() {
    let mut app = app();
    app.chrome.community_links = false;
    app.ui.tab = "Record".into();
    let mut h = harness(app);
    assert!(!text(&h).contains("Learn"));
    assert!(h.state_mut().run("app.links", json!({})).is_err());
    assert!(!control::all_commands(h.state()).to_string().contains("app.links"));
    h.state_mut().run("app.about", json!({})).unwrap();
    h.run_steps(3);
    for link in ["Source on GitHub", "Join our community on Discord", "getartcraft.com/apps/deckcraft"] {
        assert!(h.query_by_label(link).is_none());
    }
    assert!(h.query_by_label("DeckCraft").is_some());
    assert!(text(&h).contains("MIT OR Apache-2.0"));
    assert!(text(&h).contains("contributors"));
    h.state_mut().dialog = None;
    for page in ["home", "account"] {
        h.state_mut().run("app.backstage", json!({"page":page})).unwrap();
        h.run_steps(3);
        assert!(!text(&h).contains("Discord"));
        assert!(!text(&h).contains("GitHub"));
    }
}

#[test]
fn file_tab_and_start_screen_switches_cover_command_paths() {
    let mut app = app();
    app.chrome.file_tab = false;
    for command in ["app.home", "app.backstage"] {
        app.run(command, json!({"on":true,"page":"home"})).unwrap();
        assert!(app.backstage.is_none());
    }
    let mut h = harness(app);
    assert!(h.query_by_label("File").is_none());
    h.state_mut().run("file.close", json!({})).unwrap();
    h.run_steps(3);
    assert!(h.state().backstage.is_none());
    h.state_mut().chrome.file_tab = true;
    h.state_mut().chrome.start_screen = false;
    h.run_steps(3);
    assert!(h.state().backstage.is_none());
    h.state_mut().chrome.start_screen = true;
    h.run_steps(3);
    assert!(h.state().backstage.is_some());
}

#[test]
fn all_panels_setters_are_reported_and_host_chrome_is_not_persisted() {
    let ctx = egui::Context::default();
    let mut app = app();
    let report = c0_host::handle(
        &mut app,
        &ctx,
        "c0.set",
        &json!({"panels":{
            "title_bar":false,"file_tab":false,"start_screen":false,"community_links":false,"status_bar":false,
            "ribbon_collapsed":true,"notes":false,"ruler":false,
        }}),
    )
    .unwrap();
    assert_eq!(report["applied"].as_array().unwrap().len(), 8);
    let prefs = serde_json::to_value(&app.ui).unwrap();
    for key in ["title_bar", "file_tab", "start_screen", "community_links", "status_bar"] {
        assert!(prefs.get(key).is_none());
    }
    assert!(app.ui.ribbon_collapsed);
    assert!(!app.ui.notes);
    assert!(!app.ui.ruler);
}

#[test]
fn hiding_title_and_status_bars_gives_the_canvas_their_space() {
    let mut h = harness(app());
    let before = h.state().canvas_rect.unwrap();
    h.state_mut().chrome.title_bar = false;
    h.state_mut().chrome.status_bar = false;
    h.run_steps(3);
    let after = h.state().canvas_rect.unwrap();
    assert!(after.top() < before.top(), "{before:?} -> {after:?}");
    assert!(after.bottom() > before.bottom(), "{before:?} -> {after:?}");
}

#[test]
fn go_to_scrolls_a_distant_thumbnail_into_the_visible_pane() {
    fn visible(shape: &egui::Shape, clip: egui::Rect) -> bool {
        match shape {
            egui::Shape::Text(t) => t.galley.text() == "30" && clip.contains(t.pos),
            egui::Shape::Vec(v) => v.iter().any(|s| visible(s, clip)),
            _ => false,
        }
    }
    let mut app = app();
    for _ in 1..30 {
        app.run("slide.new", json!({})).unwrap();
    }
    let mut h = harness(app);
    assert!(!h.output().shapes.iter().any(|s| visible(&s.shape, s.clip_rect)));
    c0_host::handle(h.state_mut(), &egui::Context::default(), "c0.goTo", &json!({"slide":29})).unwrap();
    h.run_steps(40);
    assert_eq!(h.state().session.active().unwrap().selection.slide, 29);
    assert!(h.output().shapes.iter().any(|s| visible(&s.shape, s.clip_rect)));
}

#[test]
fn contributor_names_remain_but_both_github_link_layouts_honor_the_switch() {
    for visible in [true, false] {
        let mut app = app();
        app.chrome.community_links = visible;
        let mut dialog = deckcraft_ui_egui::dialogs::Dialog::new("about");
        dialog.fields.insert("tab".into(), "contributors".into());
        app.dialog = Some(dialog);
        let mut h = harness(app);
        for table in [false, true] {
            if table {
                h.get_by_label("Table").click();
                h.run_steps(3);
            }
            use deckcraft_ui_egui::credits::{CONTRIBUTORS, NameMode, SortKey, sorted};
            let names = sorted(CONTRIBUTORS, NameMode::Username, SortKey::FirstCommit, true);
            let name = names.first().expect("a contributor name remains visible").name(NameMode::Username);
            h.get_by_label(&name).click();
            let mut opened = false;
            for _ in 0..4 {
                h.run_steps(1);
                opened |= h
                    .output()
                    .platform_output
                    .commands
                    .iter()
                    .any(|command| matches!(command,egui::OutputCommand::OpenUrl(url) if url.url.starts_with("https://github.com/")));
            }
            assert_eq!(opened, visible, "table={table}: {name}");
        }
    }
}

#[test]
fn dirty_replacement_uses_existing_cancel_dont_save_and_failed_save_paths() {
    for answer in ["Cancel", "Don't Save", "Save"] {
        let mut app = app();
        app.services.pick_save = Some(Box::new(|_| None));
        app.run("slide.new", json!({"title":"Keep me"})).unwrap();
        let ctx = egui::Context::default();
        let uid = app.session.active().unwrap().uid;
        let reply = c0_host::handle(&mut app, &ctx, "c0.newDocument", &json!({})).unwrap();
        assert_eq!(reply, json!({"pending":"saveChanges"}));
        assert_eq!(app.session.active().unwrap().uid, uid);
        let mut h = harness(app);
        h.get_by_label(answer).click();
        h.run_steps(4);
        assert!(h.state().dialog.is_none());
        if answer == "Don't Save" {
            assert_ne!(h.state().session.active().unwrap().uid, uid);
            assert_eq!(h.state().session.active().unwrap().doc.slides.len(), 1);
        } else {
            assert_eq!(h.state().session.active().unwrap().uid, uid);
            assert!(h.state().session.active().unwrap().is_dirty());
        }
    }
}

#[test]
fn dirty_open_uses_the_same_existing_discard_prompt() {
    let mut app = app();
    let ctx = egui::Context::default();
    let snapshot = c0_host::handle(&mut app, &ctx, "c0.documentBytes", &json!({})).unwrap();
    app.run("slide.new", json!({"title":"Discard after confirmation"})).unwrap();
    let reply = c0_host::handle(&mut app, &ctx, "c0.openFile", &json!({"name":snapshot["name"],"bytes":snapshot["bytes"]})).unwrap();
    assert_eq!(reply, json!({"pending":"saveChanges"}));
    assert_eq!(app.session.active().unwrap().doc.slides.len(), 2);
    let mut h = harness(app);
    h.get_by_label("Don't Save").click();
    h.run_steps(4);
    assert_eq!(h.state().session.documents().len(), 1);
    assert_eq!(h.state().session.active().unwrap().doc.slides.len(), 1);
    assert!(!h.state().session.active().unwrap().is_dirty());
}

#[test]
fn successful_existing_save_finishes_the_new_document_request() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.run("slide.new", json!({"title":"Saved original"})).unwrap();
    let path = std::env::temp_dir().join(format!("c0deck-save-{}.deckcraft", app.session.active().unwrap().uid));
    let destination = path.to_string_lossy().to_string();
    app.services.pick_save = Some(Box::new(move |_| Some(destination.clone())));
    c0_host::handle(&mut app, &ctx, "c0.newDocument", &json!({})).unwrap();
    let mut h = harness(app);
    h.get_by_label("Save").click();
    h.run_steps(4);
    assert!(h.state().dialog.is_none());
    assert_eq!(h.state().session.active().unwrap().doc.slides.len(), 1);
    let saved = deckcraft_engine::cmd::file::open_presentation("saved.deckcraft", &std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved.slides.len(), 2);
    assert_eq!(saved.slides[1].title(), "Saved original");
    std::fs::remove_file(path).unwrap();
}
