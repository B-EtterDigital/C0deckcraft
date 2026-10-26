use deckcraft_engine::Session;
use deckcraft_ui_egui::{
    Services, SlideApp, c0_host,
    theme::{self, Tokens},
};
use serde_json::json;

#[test]
fn theme_overrides_survive_light_dark_and_clear_to_current_palette() {
    let ctx = egui::Context::default();
    ctx.set_visuals(egui::Visuals::light());
    theme::host::set(&ctx, &json!({"accent":"#123456","text":"#AABBCCDD"})).unwrap();
    assert_eq!(Tokens::get(&ctx).accent, egui::Color32::from_rgb(0x12, 0x34, 0x56));
    assert_eq!(Tokens::get(&ctx).text, egui::Color32::from_rgba_unmultiplied(0xAA, 0xBB, 0xCC, 0xDD));
    let mut app = SlideApp::new(Session::new(), Services::default());
    app.run("view.dark", json!({"on":true})).unwrap();
    // Since upstream's logic/ui split the restyle runs in a UI pass, after the fonts pass.
    for _ in 0..3 {
        ctx.run_ui(Default::default(), |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        })
        .textures_delta
        .clear();
    }
    assert!(Tokens::get(&ctx).dark);
    assert_eq!(Tokens::get(&ctx).accent, egui::Color32::from_rgb(0x12, 0x34, 0x56));
    theme::host::set(&ctx, &json!(null)).unwrap();
    assert_eq!(Tokens::get(&ctx).accent, Tokens::dark().accent);
    assert!(Tokens::get(&ctx).dark);
}

#[test]
fn every_colour_field_is_registered() {
    let ctx = egui::Context::default();
    let source = include_str!("../src/theme.rs");
    let fields: Vec<_> = source.lines().filter_map(|s| s.trim().strip_prefix("pub ").and_then(|s| s.strip_suffix(": Color32,"))).collect();
    // An upstream field addition fails here until host.rs registers it (27 on 2026-10-25).
    assert_eq!(fields.len(), 27);
    for key in fields {
        let report = theme::host::set(&ctx, &json!({key:"#123456"})).unwrap();
        assert_eq!(report.applied, [key]);
        assert!(report.unknown.is_empty());
        assert!(report.invalid.is_empty());
    }
}

#[test]
fn settings_reports_paths_and_applies_valid_siblings() {
    let ctx = egui::Context::default();
    let mut app = SlideApp::new(Session::new(), Services::default());
    let report = c0_host::handle(
        &mut app,
        &ctx,
        "c0.set",
        &json!({
            "themeTokens":{"accent":"#102030", "text":3, "dark":"#ffffff", "mystery":"#000000"},
            "panels":{"title_bar":false, "notes":false, "ruler":"yes", "mystery":true}, "mystery":false,
        }),
    )
    .unwrap();
    assert_eq!(report["applied"], json!(["themeTokens.accent", "panels.notes", "panels.title_bar"]));
    assert_eq!(report["invalid"], json!(["themeTokens.text", "panels.ruler"]));
    assert_eq!(report["unknown"], json!(["mystery", "themeTokens.dark", "themeTokens.mystery", "panels.mystery"]));
    assert!(!app.chrome.title_bar);
    assert!(!app.ui.notes);
    assert_eq!(Tokens::get(&ctx).accent, egui::Color32::from_rgb(0x10, 0x20, 0x30));
}

#[test]
fn malformed_containers_are_atomic_and_overrides_are_context_local() {
    let ctx = egui::Context::default();
    let other = egui::Context::default();
    let mut app = SlideApp::new(Session::new(), Services::default());
    let before = Tokens::get(&other).accent;
    for params in [json!({"themeTokens":3,"panels":{"title_bar":false}}), json!({"themeTokens":{"accent":"#123456"},"panels":[]})] {
        assert!(c0_host::handle(&mut app, &ctx, "c0.set", &params).is_err());
        assert!(app.chrome.title_bar);
    }
    theme::host::set(&ctx, &json!({"accent":"#123456"})).unwrap();
    assert_eq!(Tokens::get(&other).accent, before);
    for color in [json!("red"), json!("#123"), json!("#zzzzzz"), json!("#１２３４５６"), json!(null)] {
        assert_eq!(theme::host::set(&ctx, &json!({"accent":color})).unwrap().invalid, ["accent"]);
    }
    theme::host::set(&ctx, &json!({})).unwrap();
    assert_eq!(Tokens::get(&ctx).accent, before);
}
