//! Native egui search shell consuming the derived SQLite catalog.
//! Keyboard navigation respects focused controls; querying every dictionary
//! requires an explicit opt-in.

use eframe::egui;

use crate::model::DictionarySource;
use crate::reader;
use crate::search_worker::{self, SearchScope, SearchWorker};
use crate::source_manager::{ManagerChange, SourceManager};
use crate::storage::SearchHit;
use crate::theme::{self, ThemeChoice};

const THEME_KEY: &str = "lexiva.appearance.v1";
const READER_SCALE_KEY: &str = "lexiva.reader_scale.v1";
const SCOPE_KEY: &str = "lexiva.search_scope.v1";
const HEADWORD_LANGUAGE_KEY: &str = "lexiva.headword_language.v1";
const DEFINITION_LANGUAGE_KEY: &str = "lexiva.definition_language.v1";
const INTERFACE_SCALE_KEY: &str = "lexiva.interface_scale.v1";
const REDUCED_MOTION_KEY: &str = "lexiva.reduced_motion.v1";
const HIGH_CONTRAST_KEY: &str = "lexiva.high_contrast.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
struct EntryKey {
    source_id: String,
    entry_id: String,
}

impl EntryKey {
    fn from_hit(hit: &SearchHit) -> Self {
        Self {
            source_id: hit.entry.source_id.clone(),
            entry_id: hit.entry.source_entry_id.clone(),
        }
    }

    fn matches(&self, hit: &SearchHit) -> bool {
        self.source_id == hit.entry.source_id && self.entry_id == hit.entry.source_entry_id
    }
}

/// Search navigation must not intercept keys owned by any other focused
/// widget (including source-manager inputs, menus and language filters).
fn lookup_shortcuts_allowed(
    window_focused: bool,
    focused_widget: Option<egui::Id>,
    lookup_widget: egui::Id,
) -> bool {
    window_focused && focused_widget.is_none_or(|id| id == lookup_widget)
}

#[derive(Default)]
struct SearchHistory {
    queries: Vec<String>,
    cursor: Option<usize>,
}

impl SearchHistory {
    fn remember(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            return;
        }
        self.queries.retain(|previous| previous != query);
        self.queries.push(query.to_owned());
        if self.queries.len() > 40 {
            self.queries.remove(0);
        }
        self.cursor = None;
    }

    fn previous(&mut self) -> Option<&str> {
        let index = match self.cursor {
            Some(index) => index.saturating_sub(1),
            None => self.queries.len().checked_sub(1)?,
        };
        self.cursor = Some(index);
        self.queries.get(index).map(String::as_str)
    }

    fn next(&mut self) -> Option<&str> {
        let index = self.cursor?;
        if index + 1 >= self.queries.len() {
            self.cursor = None;
            return None;
        }
        self.cursor = Some(index + 1);
        self.queries.get(index + 1).map(String::as_str)
    }
}

pub struct LexivaApp {
    theme: ThemeChoice,
    darkman_watcher: Option<theme::DarkmanWatcher>,
    last_system_theme: Option<ThemeChoice>,
    reader_scale: f32,
    interface_scale: f32,
    reduced_motion: bool,
    high_contrast: bool,
    query: String,
    headword_language: String,
    definition_language: String,
    scope: SearchScope,
    worker: Option<SearchWorker>,
    manager: Option<SourceManager>,
    serial: u64,
    pending: bool,
    sources: Vec<DictionarySource>,
    hits: Vec<SearchHit>,
    suggestions: Vec<String>,
    history: SearchHistory,
    selected: Option<EntryKey>,
    error: Option<String>,
}

impl LexivaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let high_contrast = theme::high_contrast_from_storage(
            cc.storage
                .and_then(|storage| storage.get_string(HIGH_CONTRAST_KEY))
                .as_deref(),
        );
        theme::configure_palette(&cc.egui_ctx, high_contrast);
        theme::configure_system_font(&cc.egui_ctx);
        let interface_scale = theme::interface_scale_from_storage(
            cc.storage
                .and_then(|storage| storage.get_string(INTERFACE_SCALE_KEY))
                .as_deref(),
        );
        cc.egui_ctx.set_zoom_factor(interface_scale);
        let reduced_motion = theme::reduced_motion_from_storage(
            cc.storage
                .and_then(|storage| storage.get_string(REDUCED_MOTION_KEY))
                .as_deref(),
        );
        theme::configure_motion(&cc.egui_ctx, reduced_motion);
        let reader_scale = theme::reader_scale_from_storage(
            cc.storage
                .and_then(|storage| storage.get_string(READER_SCALE_KEY))
                .as_deref(),
        );
        let scope = cc
            .storage
            .and_then(|storage| storage.get_string(SCOPE_KEY))
            .map(|value| SearchScope::from_storage_value(&value))
            .unwrap_or_default();
        let headword_language = cc
            .storage
            .and_then(|storage| storage.get_string(HEADWORD_LANGUAGE_KEY))
            .unwrap_or_default();
        let definition_language = cc
            .storage
            .and_then(|storage| storage.get_string(DEFINITION_LANGUAGE_KEY))
            .unwrap_or_default();
        let theme = cc
            .storage
            .and_then(|storage| storage.get_string(THEME_KEY))
            .and_then(|value| ThemeChoice::from_storage_value(&value))
            .unwrap_or_default();
        theme.apply(&cc.egui_ctx);
        let darkman_watcher = theme::DarkmanWatcher::spawn(&cc.egui_ctx);

        let (worker, manager, error) = match search_worker::catalog_path() {
            Ok(path) => {
                // Initialize the new catalog once before independent search
                // and import connections race to create its schema.
                match crate::storage::DictionaryStore::open(&path) {
                    Ok(_) => {
                        let search = SearchWorker::spawn(path.clone(), &cc.egui_ctx);
                        let import = SourceManager::spawn(path, &cc.egui_ctx);
                        match (search, import) {
                            (Ok(search), Ok(import)) => (Some(search), Some(import), None),
                            (Err(error), _) | (_, Err(error)) => (
                                None,
                                None,
                                Some(format!("Could not start workers: {error}")),
                            ),
                        }
                    }
                    Err(error) => (None, None, Some(error.to_string())),
                }
            }
            Err(error) => (
                None,
                None,
                Some(format!("Could not open catalog folder: {error}")),
            ),
        };
        let mut app = Self {
            theme,
            darkman_watcher,
            last_system_theme: None,
            reader_scale,
            interface_scale,
            reduced_motion,
            high_contrast,
            query: String::new(),
            headword_language,
            definition_language,
            scope,
            worker,
            manager,
            serial: 0,
            pending: false,
            sources: Vec::new(),
            hits: Vec::new(),
            suggestions: Vec::new(),
            history: SearchHistory::default(),
            selected: None,
            error,
        };
        app.dispatch_search();
        app
    }

    fn dispatch_search(&mut self) {
        self.serial += 1;
        self.hits.clear();
        self.suggestions.clear();
        self.selected = None;
        if let Some(worker) = &self.worker {
            match worker.submit(
                self.serial,
                &self.query,
                &self.scope,
                &self.headword_language,
                &self.definition_language,
            ) {
                Ok(()) => {
                    self.pending = true;
                    self.error = None;
                }
                Err(error) => {
                    self.pending = false;
                    self.error = Some(error);
                }
            }
        }
    }

    fn receive_updates(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };
        if let Some(update) = worker.poll() {
            if update.serial != self.serial {
                return;
            }
            self.pending = false;
            self.sources = update.sources;
            self.hits = update.hits;
            self.suggestions = update.suggestions;
            self.error = update.error;
            // Removed or disabled sources never silently turn into an All search.
            if let SearchScope::Source(id) = &self.scope
                && !self.sources.iter().any(|source| &source.id == id)
                && self.error.is_none()
            {
                self.scope = SearchScope::Unselected;
                self.dispatch_search();
                return;
            }
            if !self
                .selected
                .as_ref()
                .is_some_and(|key| self.hits.iter().any(|hit| key.matches(hit)))
            {
                self.selected = self.hits.first().map(EntryKey::from_hit);
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.hits.is_empty() {
            return;
        }
        let selected_index = self
            .selected
            .as_ref()
            .and_then(|key| self.hits.iter().position(|hit| key.matches(hit)))
            .unwrap_or(0);
        let next = (selected_index as isize + delta).rem_euclid(self.hits.len() as isize) as usize;
        self.selected = Some(EntryKey::from_hit(&self.hits[next]));
    }

    fn render_empty(&self, ui: &mut egui::Ui) {
        ui.add_space(55.0);
        ui.vertical_centered(|ui| {
            if self.sources.is_empty() {
                ui.heading("A dictionary you control.");
                ui.label("No dictionary sources have been indexed yet.");
                ui.weak("Use the source manager above to scan for installed dictionaries or import a local file.");
            } else if self.scope == SearchScope::Unselected {
                ui.heading("Choose a dictionary to search.");
                ui.weak(
                    "No dictionaries are queried automatically. Select one source, or explicitly choose all indexed sources.",
                );
            } else if self.query.trim().is_empty() {
                ui.heading("Find a word.");
                ui.weak("Type an English or Spanish headword to search the selected index.");
            } else if !self.pending && self.error.is_none() {
                ui.heading("No matching headwords.");
                ui.weak("Try a shorter prefix or a different indexed dictionary.");
            }
        });
    }
}

impl eframe::App for LexivaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(mode) = self
            .darkman_watcher
            .as_ref()
            .and_then(theme::DarkmanWatcher::latest)
        {
            self.last_system_theme = Some(mode);
            if self.theme == ThemeChoice::System {
                mode.apply(ui.ctx());
            }
        }
        self.receive_updates();
        // Also preserve egui's native Ctrl+Plus/Minus/0 zoom shortcuts.
        self.interface_scale = ui.ctx().zoom_factor();
        let changed = self.manager.as_mut().and_then(SourceManager::poll);
        if let Some(change) = changed {
            match change {
                ManagerChange::Imported(id) => self.scope = SearchScope::Source(id),
                ManagerChange::Removed(id) => {
                    if self.scope == SearchScope::Source(id) {
                        self.scope = SearchScope::Unselected;
                    }
                }
                ManagerChange::Preferences(id) => {
                    if self.scope == SearchScope::Source(id.clone())
                        && !self
                            .manager
                            .as_ref()
                            .is_some_and(|manager| manager.is_enabled(&id))
                    {
                        self.scope = SearchScope::Unselected;
                    }
                }
            }
            self.dispatch_search();
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.heading("Lexiva");
            ui.separator();
            ui.weak("Offline dictionary reader");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let previous = self.theme;
                egui::ComboBox::from_id_salt("lexiva_appearance")
                    .selected_text(self.theme.label())
                    .show_ui(ui, |ui| {
                        for choice in ThemeChoice::ALL {
                            ui.selectable_value(&mut self.theme, choice, choice.label());
                        }
                    });
                if self.theme != previous {
                    self.theme.apply(ui.ctx());
                    if self.theme == ThemeChoice::System
                        && let Some(mode) = self.last_system_theme
                    {
                        mode.apply(ui.ctx());
                    }
                }
                ui.label("Appearance");
                ui.separator();
                ui.menu_button("Accessibility", |ui| {
                    ui.label("Interface scale");
                    let mut scale = self.interface_scale.clamp(0.75, 2.0);
                    if ui
                        .add(
                            egui::Slider::new(&mut scale, 0.75..=2.0)
                                .step_by(0.05)
                                .suffix("×"),
                        )
                        .changed()
                    {
                        self.interface_scale = scale;
                        ui.ctx().set_zoom_factor(scale);
                    }
                    if ui.button("Reset interface zoom").clicked() {
                        self.interface_scale = 1.0;
                        ui.ctx().set_zoom_factor(1.0);
                    }
                    if ui
                        .checkbox(&mut self.reduced_motion, "Reduce animations")
                        .changed()
                    {
                        theme::configure_motion(ui.ctx(), self.reduced_motion);
                    }
                    if ui
                        .checkbox(&mut self.high_contrast, "High-contrast text")
                        .changed()
                    {
                        theme::configure_palette(ui.ctx(), self.high_contrast);
                    }
                    ui.label("Ctrl+Plus/Minus/0 also adjusts interface zoom.");
                });
            });
        });
        ui.separator();

        if let Some(manager) = &mut self.manager {
            manager.show(ui);
        }
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label("Dictionary");
            let previous = self.scope.clone();
            egui::ComboBox::from_id_salt("lexiva_source_choice")
                .selected_text(self.scope.label(&self.sources))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.scope,
                        SearchScope::Unselected,
                        "Choose a dictionary",
                    );
                    ui.selectable_value(
                        &mut self.scope,
                        SearchScope::All,
                        "All indexed dictionaries (explicit)",
                    );
                    for source in &self.sources {
                        ui.selectable_value(
                            &mut self.scope,
                            SearchScope::Source(source.id.clone()),
                            &source.title,
                        );
                    }
                });
            if self.scope != previous {
                self.dispatch_search();
            }
            ui.weak(format!("{} indexed sources", self.sources.len()));
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Word language");
            let word = ui.add(
                egui::TextEdit::singleline(&mut self.headword_language)
                    .hint_text("Any (e.g. es)")
                    .desired_width(115.0),
            );
            ui.label("Definition language");
            let meaning = ui.add(
                egui::TextEdit::singleline(&mut self.definition_language)
                    .hint_text("Any (e.g. en)")
                    .desired_width(115.0),
            );
            if word.changed() || meaning.changed() {
                self.dispatch_search();
            }
        });
        ui.add_space(8.0);

        let response = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Look up a word…")
                .desired_width(f32::INFINITY),
        );
        if response.changed() {
            self.dispatch_search();
        }
        let focus_search =
            ui.input(|input| input.modifiers.ctrl && input.key_pressed(egui::Key::L));
        if focus_search {
            response.request_focus();
        }
        let submitted =
            ui.input(|input| input.key_pressed(egui::Key::Enter) && !input.modifiers.ctrl);
        if submitted && response.has_focus() {
            self.history.remember(&self.query);
        }
        let shortcut_context = lookup_shortcuts_allowed(
            ui.input(|input| input.focused),
            ui.ctx().memory(|memory| memory.focused()),
            response.id,
        );
        let copy_shortcut = shortcut_context
            && ui.input(|input| input.modifiers.ctrl && input.key_pressed(egui::Key::Enter));
        if copy_shortcut
            && let Some(hit) = self
                .selected
                .as_ref()
                .and_then(|selected| self.hits.iter().find(|hit| selected.matches(hit)))
        {
            ui.ctx().copy_text(reader::copy_text(hit));
        }

        // History is intentionally session-only for now. Ctrl+L focuses
        // lookup, Alt+Up/Down traverses submitted terms, ordinary arrows
        // select search results, and Ctrl+Enter copies the selected result.
        let history_up = shortcut_context
            && ui.input(|input| input.modifiers.alt && input.key_pressed(egui::Key::ArrowUp));
        let history_down = shortcut_context
            && ui.input(|input| input.modifiers.alt && input.key_pressed(egui::Key::ArrowDown));
        let recalled = if history_up {
            self.history.previous().map(str::to_owned)
        } else if history_down {
            self.history.next().map(str::to_owned)
        } else {
            None
        };
        if let Some(query) = recalled {
            self.query = query;
            self.dispatch_search();
        }
        if shortcut_context
            && ui.input(|input| !input.modifiers.alt && input.key_pressed(egui::Key::ArrowDown))
        {
            self.move_selection(1);
        }
        if shortcut_context
            && ui.input(|input| !input.modifiers.alt && input.key_pressed(egui::Key::ArrowUp))
        {
            self.move_selection(-1);
        }
        ui.add_space(5.0);
        ui.label("Keyboard: Ctrl+L focus lookup · ↑/↓ select results · Alt+↑/↓ history · Ctrl+Enter copy");
        if self.pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak("Searching the local index…");
            });
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Definition text size");
            ui.add(
                egui::Slider::new(&mut self.reader_scale, 0.8..=1.6)
                    .step_by(0.1)
                    .show_value(false),
            );
            ui.weak(format!("{:.0}%", self.reader_scale * 100.0));
            if ui.small_button("Reset").clicked() {
                self.reader_scale = 1.0;
            }
        });

        if self.hits.is_empty() {
            self.render_empty(ui);
            if !self.suggestions.is_empty() {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    ui.strong("Did you mean…");
                });
                let mut chosen = None;
                ui.horizontal_wrapped(|ui| {
                    for word in &self.suggestions {
                        if ui.button(word).clicked() {
                            chosen = Some(word.clone());
                        }
                    }
                });
                if let Some(word) = chosen {
                    self.history.remember(&word);
                    self.query = word;
                    self.dispatch_search();
                }
            }
            return;
        }

        ui.columns(2, |columns| {
            columns[0].heading(format!("Results ({})", self.hits.len()));
            columns[0].add_space(4.0);
            let mut clicked = None;
            egui::ScrollArea::vertical()
                .id_salt("lexiva_result_list")
                .show(&mut columns[0], |ui| {
                    for hit in &self.hits {
                        let is_selected =
                            self.selected.as_ref().is_some_and(|key| key.matches(hit));
                        let title = format!(
                            "{}{}",
                            hit.entry.lemma,
                            if hit.exact { "  ·  exact" } else { "" }
                        );
                        if ui.selectable_label(is_selected, title).clicked() {
                            clicked = Some(EntryKey::from_hit(hit));
                        }
                        ui.weak(format!(
                            "{} · {}",
                            hit.source.title,
                            hit.entry.part_of_speech.as_deref().unwrap_or("unlabeled")
                        ));
                        ui.add_space(5.0);
                    }
                });
            if let Some(clicked) = clicked {
                self.selected = Some(clicked);
            }
            columns[1].heading("Definition");
            columns[1].add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("lexiva_definition_reader")
                .show(&mut columns[1], |ui| {
                    let selected = self
                        .selected
                        .as_ref()
                        .and_then(|key| self.hits.iter().find(|hit| key.matches(hit)));
                    if let Some(hit) = selected {
                        reader::show(ui, hit, self.reader_scale);
                    } else {
                        ui.weak("Choose a search result to read its definitions.");
                    }
                });
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, self.theme.storage_value().to_owned());
        storage.set_string(READER_SCALE_KEY, format!("{:.1}", self.reader_scale));
        storage.set_string(SCOPE_KEY, self.scope.storage_value());
        storage.set_string(HEADWORD_LANGUAGE_KEY, self.headword_language.clone());
        storage.set_string(DEFINITION_LANGUAGE_KEY, self.definition_language.clone());
        storage.set_string(INTERFACE_SCALE_KEY, format!("{:.2}", self.interface_scale));
        storage.set_string(REDUCED_MOTION_KEY, self.reduced_motion.to_string());
        storage.set_string(HIGH_CONTRAST_KEY, self.high_contrast.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_navigation_does_not_capture_other_controls_or_background_windows() {
        let lookup = egui::Id::unique("lookup");
        let language = egui::Id::unique("language");
        assert!(lookup_shortcuts_allowed(true, None, lookup));
        assert!(lookup_shortcuts_allowed(true, Some(lookup), lookup));
        assert!(!lookup_shortcuts_allowed(true, Some(language), lookup));
        assert!(!lookup_shortcuts_allowed(false, None, lookup));
        assert!(!lookup_shortcuts_allowed(false, Some(lookup), lookup));
    }

    #[test]
    fn history_deduplicates_caps_and_navigates_without_guessing_sources() {
        let mut history = SearchHistory::default();
        assert_eq!(history.previous(), None);
        history.remember("  casa  ");
        history.remember("árbol");
        history.remember("casa");
        assert_eq!(history.queries, ["árbol", "casa"]);
        assert_eq!(history.previous(), Some("casa"));
        assert_eq!(history.previous(), Some("árbol"));
        assert_eq!(history.previous(), Some("árbol"));
        assert_eq!(history.next(), Some("casa"));
        assert_eq!(history.next(), None);
        for i in 0..50 {
            history.remember(&format!("item{i}"));
        }
        assert_eq!(history.queries.len(), 40);
        assert_eq!(history.queries.first().map(String::as_str), Some("item10"));
    }

    #[test]
    fn selection_is_stable_by_source_and_entry_identity() {
        let key = EntryKey {
            source_id: "one".into(),
            entry_id: "homograph-1".into(),
        };
        let different = EntryKey {
            source_id: "two".into(),
            entry_id: "homograph-1".into(),
        };
        assert_ne!(key, different);
    }
}
