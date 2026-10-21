//! Imported media is preserved, but page builds refuse explicit playback before queuing it.

use deckcraft_model::{MediaClip, Shape, ShapeId, ShapeKind, defaults};
use deckcraft_ui_egui::{
    ControlRequest, Services, SlideApp, c0_host,
    control::{self, Outcome},
};
use serde_json::json;
use std::sync::Arc;

#[test]
fn imported_restricted_media_playback_fails_clearly_only_when_codecs_are_disabled() {
    for (name, video, content_type) in [("clip-h264.mp4", true, "video/mp4"), ("clip-hevc.mp4", true, "video/mp4"), ("tone.m4a", false, "audio/mp4")]
    {
        let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../media/tests/fixtures").join(name)).unwrap();
        let mut doc = defaults::new_presentation(None);
        let media = doc.add_media(name, content_type, bytes).unwrap();
        let id = ShapeId(doc.alloc_id().unwrap());
        Arc::make_mut(doc.slides.get_mut(0).unwrap()).shapes.push(Shape {
            id,
            name: name.into(),
            kind: ShapeKind::Media(MediaClip { media, video, duration_ms: 1000, ..Default::default() }),
            ..Default::default()
        });
        let native = deckcraft_engine::cmd::file::save_bytes(&doc, "deckcraft").unwrap();
        let ctx = egui::Context::default();
        let mut app = SlideApp::new(deckcraft_engine::Session::new(), Services::default());
        c0_host::handle(&mut app, &ctx, "c0.openFile", &json!({"name":"restricted.deckcraft","bytes":deckcraft_engine::cmd::base64_encode(&native)}))
            .unwrap();
        let (request, _) = ControlRequest::new("engine.execute", json!({"command":"media.play","params":{"id":id.0}}));
        let Outcome::Done(reply) = control::handle(&mut app, &ctx, &request) else { panic!("unexpected deferred playback") };
        assert_eq!(reply["ok"], deckcraft_media::c0_codecs::enabled("H.264"), "{name}: {reply}");
        if !deckcraft_media::c0_codecs::enabled("H.264") {
            assert!(reply["error"].as_str().unwrap().contains("this format is not supported here"));
            assert!(app.status.as_ref().unwrap().0.contains("this format is not supported here"));
            assert!(app.dialog.is_none());
        }
        app.run("media.pause", json!({"id":id.0})).unwrap();
        app.run("media.stop", json!({"id":id.0})).unwrap();
    }
}
