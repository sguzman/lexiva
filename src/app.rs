use eframe::egui;

use crate::theme::{self, ThemeChoice};

const THEME_KEY: &str = "lexiva.appearance.v1";

/// Native application shell. Dictionary search/import remain explicitly unimplemented.
pub struct LexivaApp {
    theme: ThemeChoice,
    query: String,
}

impl LexivaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::configure_palette(&cc.egui_ctx);
        let theme = cc
            .storage
            .and_then(|storage| storage.get_string(THEME_KEY))
            .and_then(|value| ThemeChoice::from_storage_value(&value))
            .unwrap_or_default();
        theme.apply(&cc.egui_ctx);

        Self {
            theme,
            query: String::new(),
        }
    }
}

impl eframe::App for LexivaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.add_space(12.0);
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
                if previous != self.theme {
                    self.theme.apply(ui.ctx());
                }
                ui.label("Appearance");
            });
        });
        ui.separator();
        ui.add_space(22.0);

        ui.horizontal(|ui| {
            ui.add_sized(
                [ui.available_width(), 38.0],
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Search a word (available after adding dictionaries)"),
            );
        });

        ui.add_space(70.0);
        ui.vertical_centered(|ui| {
            ui.heading("A dictionary you control.");
            ui.add_space(10.0);
            ui.label("No dictionary sources are configured yet.");
            ui.add_space(6.0);
            ui.weak("GNU DICT / FreeDict and Wiktionary importers are next on the roadmap.");
            if !self.query.is_empty() {
                ui.add_space(12.0);
                ui.label("Search is not available until a dictionary importer is implemented.");
            }
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, self.theme.storage_value().to_owned());
    }
}
