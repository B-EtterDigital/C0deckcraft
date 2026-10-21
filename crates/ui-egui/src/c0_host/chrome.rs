//! Draw-site gates and the existing Backstage lifecycle, with host policy applied first.
use crate::{SlideApp, backstage};

impl SlideApp {
    /// Opens File's view, keeping its current state. A host can own both entry points instead.
    pub fn open_backstage(&mut self, page: backstage::Page) {
        if !self.chrome.file_tab || (!self.chrome.start_screen && self.session.active().is_none()) {
            return;
        }
        let key = self.docs_key();
        match &mut self.backstage {
            Some(b) => b.go(page),
            None => self.backstage = Some(backstage::Backstage::new(page, key)),
        }
    }

    pub(crate) fn sync_backstage(&mut self) {
        if !self.chrome.file_tab || (!self.chrome.start_screen && self.session.active().is_none()) {
            self.backstage = None;
            return;
        }
        let key = self.docs_key();
        let has_doc = self.session.active().is_some();
        let stale = self.backstage.as_ref().is_some_and(|b| b.docs != key);
        if has_doc && stale {
            self.backstage = None;
        } else if !has_doc && (stale || self.backstage.is_none()) {
            self.backstage = Some(backstage::Backstage::new(backstage::Page::Home, key));
        }
    }
}

pub(crate) fn title_bar(app: &mut SlideApp, ui: &mut egui::Ui) {
    if app.chrome.title_bar {
        crate::ribbon::title_bar(app, ui);
    }
}
pub(crate) fn status_bar(app: &mut SlideApp, ui: &mut egui::Ui) {
    if app.chrome.status_bar {
        crate::status::status_bar(app, ui);
    }
}
pub(crate) fn community_link(app: &SlideApp, ui: &mut egui::Ui, label: &str, url: &str) {
    if app.chrome.community_links {
        ui.hyperlink_to(label, url);
    }
}
pub(crate) fn about_links(app: &SlideApp, ui: &mut egui::Ui) {
    use deckcraft_engine::links;
    community_link(app, ui, "Join our community on Discord", links::DISCORD);
    community_link(app, ui, "getartcraft.com/apps/deckcraft", links::APP_PAGE);
    community_link(app, ui, "Source on GitHub", links::GITHUB);
}

pub(crate) fn account_links(app: &SlideApp, ui: &mut egui::Ui) {
    use deckcraft_engine::links;
    community_link(app, ui, "Join our community on Discord", links::DISCORD);
    community_link(app, ui, "Source on GitHub", links::GITHUB);
}

pub(crate) fn learn_more(app: &mut SlideApp, ui: &mut egui::Ui) {
    if app.chrome.community_links {
        crate::widgets::group(ui, |ui| {
            if crate::widgets::big_button(ui, crate::icons::Icon::Help, "Learn\nMore", true).clicked() {
                app.open_url(deckcraft_engine::links::APP_PAGE);
            }
        });
    }
}

pub(crate) fn palette<T>(app: &SlideApp, items: Vec<(String, String, T)>) -> Vec<(String, String, T)> {
    items.into_iter().filter(|(id, _, _)| app.chrome.community_links || id != "app.links").collect()
}
