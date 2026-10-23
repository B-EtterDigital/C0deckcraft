//! Tool modes for the ribbon, and a foldable right dock. Host state: never saved in a deck or in
//! preferences, and every default is the upstream layout.
//!
//! DeckCraft's tools are one ribbon: a tab row over a command card of fixed height whose groups
//! run left to right, so there is no column to count. The three PhotoCraft names therefore read:
//! `toolbar` shows the command card (off: tabs only), `toolbar_double` draws the ribbon compact
//! (a scaled card in the same dock), `toolbar_floating` lifts it into a movable strip over the
//! canvas. Compact and floating combine.
use egui::emath::TSTransform;
use egui::{Area, CornerRadius, Frame, Id, LayerId, Margin, Order, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::icons::{self, Icon};
use crate::theme::Tokens;
use crate::widgets::RIBBON_H;
use crate::{SlideApp, ViewMode};

const COMPACT: f32 = 0.8;
const GRIP_H: f32 = 16.0;
const FLOAT_W: f32 = 1100.0;
const STRIP_W: f32 = 36.0;
const BUTTON: f32 = 28.0;
/// Where the strip starts when it has never been moved: below the title bar.
const FLOAT_TOP: f32 = 56.0;

/// The task panes `view.pane` knows, with the icon the folded dock gives each.
const PANES: [(&str, &str, Icon); 9] = [
    ("format", "Format Shape", Icon::ShapeFill),
    ("background", "Format Background", Icon::FormatBackground),
    ("animation", "Animation Pane", Icon::AnimationPane),
    ("selection", "Selection", Icon::SelectionPane),
    ("comments", "Comments", Icon::ShowComments),
    ("designer", "Design Ideas", Icon::DesignIdeas),
    ("smartart", "Text Pane", Icon::SmartArt),
    ("thesaurus", "Thesaurus", Icon::Thesaurus),
    ("revisions", "Revisions", Icon::NewComment),
];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tools {
    pub toolbar: bool,
    pub double: bool,
    pub floating: bool,
    pub dock_collapsed: bool,
    /// Top-left of the floating strip in points; `None` until it is moved.
    pos: Option<Pos2>,
    /// The task pane that was open when the dock folded, so unfolding brings it back.
    folded: Option<String>,
}

impl Default for Tools {
    fn default() -> Self {
        Self { toolbar: true, double: false, floating: false, dock_collapsed: false, pos: None, folded: None }
    }
}

/// Keeps the open task pane in step with the dock after a host switch: folding hides the pane,
/// unfolding brings it back.
pub(crate) fn settle(app: &mut SlideApp) {
    let tools = &mut app.host.tools;
    if tools.dock_collapsed {
        if let Some(pane) = app.ui.pane.take() {
            tools.folded = Some(pane);
        }
    } else if let Some(pane) = tools.folded.take() {
        app.ui.pane = app.ui.pane.take().or(Some(pane));
    }
}

/// The mode switches for `c0.inspect`.
pub(crate) fn report(app: &SlideApp, panes: &mut Value) {
    let tools = &app.host.tools;
    panes["toolbar"] = json!(tools.toolbar);
    panes["toolbar_double"] = json!(tools.double);
    panes["toolbar_floating"] = json!(tools.floating);
    panes["right_dock_collapsed"] = json!(tools.dock_collapsed);
}

/// Where the floating strip sits, or null.
pub(crate) fn float_pos(app: &SlideApp) -> Value {
    json!({"pos": app.host.tools.pos.map(|p| [p.x, p.y])})
}

/// Ribbon, status bar and the folded dock's strip, in the order upstream draws them.
pub(crate) fn bars(app: &mut SlideApp, ui: &mut Ui) {
    let saved = app.ui.ribbon_collapsed;
    let hidden = !app.host.tools.toolbar;
    // The host owns the card while it is off: tab clicks cannot bring it back.
    app.ui.ribbon_collapsed |= hidden;
    match (app.host.tools.floating, app.host.tools.double) {
        (true, double) => float(app, ui, double),
        (false, true) => compact(app, ui),
        (false, false) => crate::ribbon::show(app, ui),
    }
    if hidden {
        app.ui.ribbon_collapsed = saved;
    }
    super::status_bar(app, ui);
    strip(app, ui);
}

/// What `ribbon::show` reserves, in points.
fn ribbon_height(app: &SlideApp) -> f32 {
    28.0 + if app.ui.ribbon_collapsed { 4.0 } else { RIBBON_H + 20.0 }
}

fn compact(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let h = ribbon_height(app) * COMPACT;
    egui::Panel::top("c0_ribbon_compact").exact_size(h).frame(Frame::NONE.fill(t.chrome)).show(ui, |ui| {
        let rect = ui.max_rect();
        scaled(app, ui, "c0_ribbon_compact_area", rect.min, rect.width() / COMPACT, COMPACT, None);
    });
}

fn float(app: &mut SlideApp, ui: &mut Ui, double: bool) {
    let screen = ui.ctx().content_rect();
    let scale = if double { COMPACT } else { 1.0 };
    let width = (screen.width() - 24.0).clamp(240.0, FLOAT_W);
    let start = app.host.tools.pos.unwrap_or_else(|| pos2(screen.min.x + 12.0, screen.min.y + FLOAT_TOP));
    let size = vec2(width, (ribbon_height(app) + GRIP_H) * scale);
    let origin = pos2(
        start.x.clamp(screen.min.x, (screen.max.x - 80.0).max(screen.min.x)),
        start.y.clamp(screen.min.y, (screen.max.y - size.y.min(GRIP_H * scale)).max(screen.min.y)),
    );
    let moved = scaled(app, ui, "c0_ribbon_float", origin, width / scale, scale, Some(origin));
    app.host.tools.pos = moved.or(app.host.tools.pos.map(|_| origin));
}

/// Draws the unchanged ribbon inside a layer of its own, scaled and placed in screen space. With
/// `grip` the layer is a floating strip: a handle row drags it and the new position is returned.
fn scaled(app: &mut SlideApp, ui: &Ui, key: &str, origin: Pos2, width: f32, scale: f32, grip: Option<Pos2>) -> Option<Pos2> {
    let ctx = ui.ctx();
    let t = Tokens::get(ctx);
    let id = Id::new(key);
    ctx.set_transform_layer(LayerId::new(Order::Middle, id), TSTransform::new(origin.to_vec2(), scale));
    let mut moved = None;
    let height = ribbon_height(app) + if grip.is_some() { GRIP_H } else { 0.0 };
    Area::new(id).order(Order::Middle).fixed_pos(Pos2::ZERO).constrain(false).show(ctx, |ui| {
        ui.set_clip_rect(Rect::from_min_size(Pos2::ZERO, vec2(width, height)));
        let frame = if grip.is_some() {
            Frame::NONE.fill(t.chrome).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(8))
        } else {
            Frame::NONE
        };
        frame.show(ui, |ui| {
            ui.set_width(width);
            if let Some(here) = grip {
                let (rect, resp) = ui.allocate_exact_size(vec2(width, GRIP_H), Sense::click_and_drag());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Move ribbon"));
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "· · ·", crate::theme::font(11.0), t.text_faint);
                if resp.dragged() {
                    // The pointer delta is in screen points whatever the layer's scale.
                    moved = Some(here + ui.ctx().input(|i| i.pointer.delta()));
                }
            }
            crate::ribbon::show(app, ui);
        });
    });
    moved
}

/// The folded dock: a narrow column with one icon per task pane. An icon opens its pane, which
/// unfolds the dock, as a ribbon tab unfolds the ribbon. A pane opened any other way does too.
fn strip(app: &mut SlideApp, ui: &mut Ui) {
    if !app.host.tools.dock_collapsed || matches!(app.ui.view, ViewMode::Sorter | ViewMode::NotesPage) {
        return;
    }
    if app.ui.pane.is_some() {
        app.host.tools.dock_collapsed = false;
        app.host.tools.folded = None;
        return;
    }
    let t = Tokens::get(ui.ctx());
    let mut open: Option<Option<String>> = None;
    let active = app.host.tools.folded.clone();
    egui::Panel::right("c0_dock_strip")
        .exact_size(STRIP_W)
        .resizable(false)
        .frame(Frame::NONE.fill(t.card).inner_margin(Margin::same(4)))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let button = |ui: &mut Ui, icon: Icon, label: &str, on: bool| {
                let (rect, resp) = ui.allocate_exact_size(vec2(BUTTON, BUTTON), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
                if on || resp.hovered() {
                    ui.painter().rect_filled(rect, CornerRadius::same(4), if on { t.selected } else { t.hover });
                }
                icons::paint(ui.painter(), rect.shrink(5.0), icon, t.text_dim, false);
                resp.on_hover_text(label).clicked()
            };
            if button(ui, Icon::ChevronLeft, "Unfold panels", false) {
                open = Some(active.clone());
            }
            ui.separator();
            for (id, label, icon) in PANES {
                if button(ui, icon, label, active.as_deref() == Some(id)) {
                    open = Some(Some(id.to_string()));
                }
            }
        });
    if let Some(pane) = open {
        app.ui.pane = pane;
        app.host.tools.dock_collapsed = false;
        app.host.tools.folded = None;
    }
}
